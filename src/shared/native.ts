import type { NativeBridge } from './contracts';

const unavailable = async (): Promise<never> => {
  throw {
    code: 'native_unavailable',
    message: 'Откройте Converter как приложение, чтобы работать с файлами.',
  };
};
export const nativeBridge: NativeBridge = {
  listFormats: unavailable,
  pickSources: unavailable,
  pickOutput: unavailable,
  createJob: unavailable,
  startJob: unavailable,
  pauseDownload: unavailable,
  resumeDownload: unavailable,
  cancelJob: unavailable,
  updateJob: unavailable,
  getJobs: unavailable,
  readDiagnostics: unavailable,
  exportResult: unavailable,
  openResult: unavailable,
  onJobEvent: unavailable,
};
