//! 离线同步层出口：退避、发件箱、`.sch` 离线包、投递 worker。

pub mod backoff;
pub mod outbox;
pub mod sch_package;
pub mod worker;
