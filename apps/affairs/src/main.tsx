import { App } from './App';
import { mountApp } from '@shared/lib/mount';
import { APP_TARGET } from './app-target';
import '@shared/styles/index.css';

mountApp(APP_TARGET, <App />);
