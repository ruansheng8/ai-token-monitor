const API_PORT = 19362;

const isTauriRuntime = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const apiUrl = (path: string) => {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const apiPath = normalizedPath.startsWith('/api/') || normalizedPath === '/api'
    ? normalizedPath
    : `/api${normalizedPath}`;

  if (isTauriRuntime()) {
    return `http://127.0.0.1:${API_PORT}${apiPath}`;
  }

  return apiPath;
};

export const readJsonResponse = async <T>(response: Response): Promise<T> => {
  const text = await response.text();
  const trimmed = text.trimStart();

  if (trimmed.startsWith('<!doctype') || trimmed.startsWith('<html')) {
    throw new Error('API 返回了 HTML 页面，说明请求没有命中本地后台服务。');
  }

  return JSON.parse(text) as T;
};
