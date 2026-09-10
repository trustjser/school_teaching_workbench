import { useCallback, useMemo, useRef, useState } from 'react';
import { AlertTriangle, Download, FileSpreadsheet, Upload } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { ProgressBar } from '@/components/ui/ProgressBar';
import { Table, type TableColumn } from '@/components/ui/Table';
import { Input } from '@/components/ui/Input';
import { buildStudentTemplate, parseStudentFile } from '@/lib/excel';
import { exportSheetsToXlsx } from '@/lib/exporter';
import { studentBatchImport, studentList } from '@/lib/db';
import { useAppStore } from '@/store/useAppStore';
import { useStudentStore } from '@/store/useStudentStore';
import type { ParsedStudentRow } from '@/types/api';
import type { ImportRowError } from '@/types/api';
import type { ClassContext } from '@/types/models';

export interface StudentImportDialogProps {
  open: boolean;
  onClose: () => void;
  /** 班级上下文（教务端目录 / 班级端绑定班级），用于为整批落位 classId */
  classContext?: ClassContext | null;
  /** 导入完成回调 */
  onImported?: (successRows: number, failedRows: number) => void;
}

type Step = 'pick' | 'map' | 'preview' | 'importing' | 'done';

const FIELD_LABELS: Record<string, string> = {
  studentNo: '学号',
  name: '姓名',
  gender: '性别',
  grade: '年级',
  className: '班级',
  seatNo: '座位号',
  phone: '家长电话',
  note: '备注',
};

/** 名册导入弹窗：选择文件 → 列映射预览 → 校验报告 → 确认导入 */
export function StudentImportDialog({
  open,
  onClose,
  classContext,
  onImported,
}: StudentImportDialogProps): JSX.Element {
  const app = useAppStore();
  const loadStudents = useStudentStore((s) => s.load);
  const [step, setStep] = useState<Step>('pick');
  const [fileName, setFileName] = useState('');
  const [encoding, setEncoding] = useState('');
  const [headers, setHeaders] = useState<string[]>([]);
  const [columnMap, setColumnMap] = useState<Record<string, number>>({});
  const [rows, setRows] = useState<ParsedStudentRow[]>([]);
  const [batchName, setBatchName] = useState('');
  const [progress, setProgress] = useState(0);
  const [result, setResult] = useState<{ success: number; failed: number } | null>(null);
  const [errorList, setErrorList] = useState<ImportRowError[]>([]);
  const [existingStudents, setExistingStudents] = useState<import('@/types/models').Student[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const reset = useCallback(() => {
    setStep('pick');
    setFileName('');
    setEncoding('');
    setHeaders([]);
    setColumnMap({});
    setRows([]);
    setBatchName('');
    setProgress(0);
    setResult(null);
    setErrorList([]);
    setExistingStudents([]);
  }, []);

  const handleClose = useCallback(() => {
    reset();
    onClose();
  }, [onClose, reset]);

  const handleFile = useCallback(
    async (file: File | null) => {
      if (!file) return;
      try {
        const parsed = await parseStudentFile(file, {
          defaultClassName: classContext?.className ?? null,
          defaultGrade: classContext?.grade ?? null,
        });
        setFileName(file.name);
        setEncoding(parsed.encoding);
        setHeaders(parsed.headers);
        setColumnMap(parsed.columnMap);
        setRows(parsed.parsed);
        setErrorList(parsed.parsed.flatMap((r) => r.errors));
        setBatchName(`${file.name.replace(/\.[^.]+$/, '')} 导入`);
        // 在预览阶段同时读取当前名册，用稳定学号计算变更摘要。
        const existing = await studentList({ classId: classContext?.classId ?? null });
        setExistingStudents(existing);
        setStep('map');
      } catch (err) {
        app.toastError(err, '解析文件失败');
      }
    },
    [app, classContext?.classId],
  );

  const validRows = useMemo(() => rows.filter((r) => r.errors.length === 0), [rows]);
  const invalidRows = useMemo(() => rows.filter((r) => r.errors.length > 0), [rows]);
  const rosterDiff = useMemo(() => {
    const existingByNo = new Map(existingStudents.map((s) => [s.studentNo, s]));
    const incomingByNo = new Map(validRows.map((r) => [r.studentNo, r]));
    let added = 0;
    let moved = 0;
    let updated = 0;
    validRows.forEach((row) => {
      const old = existingByNo.get(row.studentNo);
      if (!old) added += 1;
      else if ((old.className ?? '') !== row.className || (old.grade ?? '') !== row.grade) moved += 1;
      else if (old.name !== row.name || (old.seatNo ?? null) !== (row.seatNo ?? null)) updated += 1;
    });
    const removed = existingStudents.filter((s) => !incomingByNo.has(s.studentNo)).length;
    return { added, moved, updated, removed };
  }, [existingStudents, validRows]);

  const columnMapOptions = useMemo(
    () => [
      { value: '-1', label: '（忽略）' },
      ...headers.map((h, i) => ({
        value: String(i),
        label: h ? `${i + 1}. ${h}` : `第 ${i + 1} 列`,
      })),
    ],
    [headers],
  );

  const handleConfirmImport = useCallback(async () => {
    if (validRows.length === 0) return;
    setStep('importing');
    setProgress(10);
    try {
      const report = await studentBatchImport(
        validRows.map((r) => ({
          studentNo: r.studentNo,
          name: r.name,
          gender: r.gender,
          grade: r.grade || classContext?.grade || null,
          className: r.className || classContext?.className || null,
          classId: classContext?.classId ?? null,
          seatNo: r.seatNo,
          phone: r.phone || null,
          note: r.note || null,
        })),
        batchName || '名册导入',
      );
      setProgress(100);
      setResult({ success: report.successRows, failed: report.failedRows });
      setErrorList(report.errors);
      setStep('done');
      await loadStudents();
      onImported?.(report.successRows, report.failedRows);
    } catch (err) {
      app.toastError(err, '导入失败');
      setStep('preview');
    }
  }, [app, batchName, loadStudents, onImported, validRows]);

  const handleDownloadTemplate = useCallback(async () => {
    try {
      await exportSheetsToXlsx('名册导入模板.xlsx', [
        { name: '名册', columns: buildStudentTemplate()[0] as string[], rows: buildStudentTemplate().slice(1) },
      ]);
    } catch (err) {
      app.toastError(err, '下载模板失败');
    }
  }, [app]);

  const previewColumns: TableColumn<ParsedStudentRow>[] = [
    { key: 'row', header: '行号', accessor: (r) => r.rowIndex, widthClass: 'w-20' },
    { key: 'studentNo', header: '学号', accessor: (r) => r.studentNo },
    { key: 'name', header: '姓名', accessor: (r) => r.name },
    { key: 'gender', header: '性别', accessor: (r) => (r.gender === 'male' ? '男' : r.gender === 'female' ? '女' : '未知') },
    { key: 'grade', header: '年级', accessor: (r) => r.grade },
    { key: 'className', header: '班级', accessor: (r) => r.className },
    { key: 'seatNo', header: '座位号', accessor: (r) => r.seatNo ?? '' },
    {
      key: 'errors',
      header: '校验结果',
      render: (r) =>
        r.errors.length === 0 ? (
          <span className="font-semibold text-green-700">✓ 通过</span>
        ) : (
          <span className="text-red-700">{r.errors.map((e) => e.message).join('；')}</span>
        ),
    },
  ];

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title="导入学生名册"
      description="支持 .xlsx / .xls / .csv（自动识别 UTF-8 / GBK 编码）"
      widthClass="max-w-5xl"
      footer={
        <>
          <Button variant="secondary" onClick={handleClose}>
            关闭
          </Button>
          {step === 'map' && (
            <Button onClick={() => setStep('preview')} disabled={rows.length === 0}>
              下一步：校验预览
            </Button>
          )}
          {step === 'preview' && (
            <>
              <Button variant="secondary" onClick={() => setStep('map')}>
                返回列映射
              </Button>
              <Button onClick={handleConfirmImport} disabled={validRows.length === 0}>
                确认导入 {validRows.length} 行
              </Button>
            </>
          )}
        </>
      }
    >
      {step === 'pick' && (
        <div className="flex flex-col items-center gap-5 py-6">
          <FileSpreadsheet className="h-16 w-16 text-brand-600" aria-hidden />
          <p className="text-lg text-ink">选择名册文件，或将文件拖拽到此处</p>
          <input
            ref={fileInputRef}
            type="file"
            accept=".xlsx,.xls,.csv,.txt"
            className="hidden"
            onChange={(e) => {
              void handleFile(e.target.files?.[0] ?? null);
            }}
          />
          <div className="flex gap-3">
            <Button icon={<Upload className="h-5 w-5" />} onClick={() => fileInputRef.current?.click()}>
              选择文件
            </Button>
            <Button variant="secondary" icon={<Download className="h-5 w-5" />} onClick={handleDownloadTemplate}>
              下载模板
            </Button>
          </div>
          <p className="max-w-xl text-center text-sm text-ink-muted">
            模板包含列：学号、姓名、性别、年级、班级、座位号、家长电话、备注。
            学号 + 年级 + 班级 组合唯一，重复行会被标记为错误。
          </p>
        </div>
      )}

      {step === 'map' && (
        <div className="space-y-4">
          <div className="flex flex-wrap items-center gap-3 rounded-lg bg-surface-muted px-4 py-3">
            <span className="text-base font-semibold text-ink">文件：{fileName}</span>
            <span className="text-sm text-ink-muted">编码：{encoding}</span>
            <span className="text-sm text-ink-muted">解析到 {rows.length} 行</span>
          </div>
          <div className="grid grid-cols-1 gap-4 board:grid-cols-2">
            {Object.keys(FIELD_LABELS).map((field) => (
              <Select
                key={field}
                label={FIELD_LABELS[field]}
                options={columnMapOptions}
                value={String(columnMap[field] ?? -1)}
                onChange={(e) =>
                  setColumnMap((m) => ({ ...m, [field]: Number.parseInt(e.target.value, 10) }))
                }
              />
            ))}
          </div>
        </div>
      )}

      {step === 'preview' && (
        <div className="space-y-4">
          <div className="grid grid-cols-3 gap-4">
            <div className="rounded-lg border border-surface-border p-3">
              <p className="text-sm text-ink-muted">总行数</p>
              <p className="text-2xl font-bold text-ink">{rows.length}</p>
            </div>
            <div className="rounded-lg border border-green-300 bg-green-50 p-3">
              <p className="text-sm text-green-800">校验通过</p>
              <p className="text-2xl font-bold text-green-800">{validRows.length}</p>
            </div>
            <div className="rounded-lg border border-red-300 bg-red-50 p-3">
              <p className="text-sm text-red-800">错误行</p>
              <p className="text-2xl font-bold text-red-800">{invalidRows.length}</p>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <DiffStat label="新增学生" value={rosterDiff.added} tone="text-green-700" />
            <DiffStat label="转班变化" value={rosterDiff.moved} tone="text-amber-700" />
            <DiffStat label="信息更新" value={rosterDiff.updated} tone="text-blue-700" />
            <DiffStat label="名单缺失" value={rosterDiff.removed} tone="text-red-700" />
          </div>
          <p className="text-sm text-ink-muted">变更摘要仅按学号匹配；名单缺失的学生不会被自动删除，会保留并可在确认后单独处理。</p>
          <Input
            label="导入批次名"
            value={batchName}
            onChange={(e) => setBatchName(e.target.value)}
            placeholder="如：2026春-三年级二班名册"
          />
          {invalidRows.length > 0 && (
            <div className="rounded-lg border border-amber-400 bg-amber-50 p-3">
              <p className="flex items-center gap-2 font-semibold text-amber-900">
                <AlertTriangle className="h-5 w-5" aria-hidden />
                以下 {invalidRows.length} 行将被跳过，请修正后重新导入
              </p>
              <ul className="mt-2 max-h-32 list-disc space-y-1 overflow-y-auto pl-6 text-sm text-amber-900">
                {errorList.slice(0, 50).map((e, i) => (
                  <li key={`${e.row}-${e.field}-${i}`}>
                    第 {e.row} 行 · {FIELD_LABELS[e.field] ?? e.field}：{e.message}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <Table
            columns={previewColumns}
            data={rows}
            rowKey={(r) => String(r.rowIndex)}
            maxHeightClass="max-h-[40vh]"
          />
        </div>
      )}

      {step === 'importing' && (
        <div className="py-10">
          <ProgressBar value={progress} label="正在写入本地数据库（事务）" />
          <p className="mt-3 text-center text-base text-ink-muted">
            导入过程为单事务，任一行失败将整体回滚
          </p>
        </div>
      )}

      {step === 'done' && result && (
        <div className="space-y-4 py-4">
          <div className="rounded-lg border border-green-300 bg-green-50 p-5 text-center">
            <p className="text-2xl font-bold text-green-800">导入完成</p>
            <p className="mt-2 text-base text-green-900">
              成功 {result.success} 行
              {result.failed > 0 && ` · 失败 ${result.failed} 行`}
            </p>
          </div>
          {result.failed > 0 && errorList.length > 0 && (
            <ul className="max-h-40 list-disc space-y-1 overflow-y-auto rounded-lg border border-amber-400 bg-amber-50 p-4 pl-6 text-sm text-amber-900">
              {errorList.slice(0, 100).map((e, i) => (
                <li key={`${e.row}-${e.field}-${i}`}>
                  第 {e.row} 行 · {FIELD_LABELS[e.field] ?? e.field}：{e.message}
                </li>
              ))}
            </ul>
          )}
          <div className="flex justify-center">
            <Button
              onClick={() => {
                void studentList().then(() => loadStudents());
                handleClose();
              }}
            >
              完成
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

function DiffStat({ label, value, tone }: { label: string; value: number; tone: string }): JSX.Element {
  return <div className="rounded-lg border border-surface-border bg-surface-muted p-3"><p className="text-sm text-ink-muted">{label}</p><p className={`text-2xl font-bold ${tone}`}>{value}</p></div>;
}
