import React, { useState, useEffect, useRef } from 'react';
import { apiUrl, isTauriRuntime } from '../lib/api';

export interface Skill {
  id: string;
  name: string;
  description: string;
  is_builtin: boolean;
}

interface SkillManagerModalProps {
  isOpen: boolean;
  onClose: () => void;
  onRefreshSkills: () => void;
}

export function SkillManagerModal({ isOpen, onClose, onRefreshSkills }: SkillManagerModalProps) {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [uploading, setUploading] = useState<boolean>(false);
  const [dragActive, setDragActive] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [successSkills, setSuccessSkills] = useState<Skill[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);

  interface FileWithRelativePath extends File {
    relativePath?: string;
  }

  const fetchSkills = async () => {
    setLoading(true);
    setErrorMsg(null);
    try {
      const res = await fetch(apiUrl('/review/skills'));
      if (res.ok) {
        const data = await res.json();
        setSkills(data);
      } else {
        setErrorMsg('获取技能列表失败');
      }
    } catch (e) {
      setErrorMsg('网络异常，获取技能列表失败');
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    setSuccessSkills([]);
    let timer: any = null;
    if (isOpen) {
      timer = setTimeout(() => {
        fetchSkills();
      }, 0);
    }
    return () => {
      if (timer) clearTimeout(timer);
    };
  }, [isOpen]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    if (isOpen && isTauriRuntime()) {
      import('@tauri-apps/api/webviewWindow')
        .then(({ getCurrentWebviewWindow }) => {
          const appWindow = getCurrentWebviewWindow();
          appWindow
            .onDragDropEvent((event) => {
              if (event.payload.type === 'enter' || event.payload.type === 'over') {
                setDragActive(true);
              } else if (event.payload.type === 'leave') {
                setDragActive(false);
              } else if (event.payload.type === 'drop') {
                setDragActive(false);
                if (event.payload.paths && event.payload.paths.length > 0) {
                  importPaths(event.payload.paths);
                }
              }
            })
            .then((unlistenFn) => {
              unlisten = unlistenFn;
            });
        })
        .catch((err) => {
          console.error('Failed to load Tauri webviewWindow API:', err);
        });
    }

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [isOpen]);

  if (!isOpen) return null;

  const handleDrag = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setDragActive(true);
    } else if (e.type === 'dragleave') {
      setDragActive(false);
    }
  };

  const traverseFileTree = async (item: DataTransferItem): Promise<FileWithRelativePath[]> => {
    const entry = item.webkitGetAsEntry();
    if (!entry) return [];

    const files: FileWithRelativePath[] = [];

    const traverse = async (entry: FileSystemEntry, path: string = ""): Promise<void> => {
      if (entry.isFile) {
        await new Promise<void>((resolve) => {
          (entry as FileSystemFileEntry).file((file) => {
            const fileWithPath = file as FileWithRelativePath;
            fileWithPath.relativePath = path + entry.name;
            files.push(fileWithPath);
            resolve();
          }, () => resolve());
        });
      } else if (entry.isDirectory) {
        const dirReader = (entry as FileSystemDirectoryEntry).createReader();
        const readEntries = (): Promise<FileSystemEntry[]> => {
          return new Promise((resolve) => {
            dirReader.readEntries((entries) => resolve(entries), () => resolve([]));
          });
        };

        let entries = await readEntries();
        const allEntries: FileSystemEntry[] = [];
        while (entries.length > 0) {
          allEntries.push(...entries);
          entries = await readEntries();
        }

        for (const e of allEntries) {
          await traverse(e, path + entry.name + "/");
        }
      }
    };

    await traverse(entry);
    return files;
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(false);
    setErrorMsg(null);

    if (e.dataTransfer.items && e.dataTransfer.items.length > 0) {
      setUploading(true);
      try {
        const filePromises: Promise<FileWithRelativePath[]>[] = [];
        for (let i = 0; i < e.dataTransfer.items.length; i++) {
          const item = e.dataTransfer.items[i];
          if (item.kind === 'file') {
            filePromises.push(traverseFileTree(item));
          }
        }
        const fileArrays = await Promise.all(filePromises);
        const files = fileArrays.flat();
        if (files.length > 0) {
          await uploadFiles(files);
        }
      } catch (err) {
        setErrorMsg('解析拖入的文件/文件夹失败');
        console.error(err);
      } finally {
        setUploading(false);
      }
    } else if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      const filesArray = Array.from(e.dataTransfer.files) as FileWithRelativePath[];
      await uploadFiles(filesArray);
    }
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    setErrorMsg(null);
    if (e.target.files && e.target.files.length > 0) {
      const filesArray = Array.from(e.target.files) as FileWithRelativePath[];
      await uploadFiles(filesArray);
    }
  };

  const handleFolderChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    setErrorMsg(null);
    if (e.target.files && e.target.files.length > 0) {
      const filesArray = Array.from(e.target.files) as FileWithRelativePath[];
      await uploadFiles(filesArray);
    }
  };

  const uploadFiles = async (files: FileWithRelativePath[]) => {
    if (files.length === 0) return;

    setUploading(true);
    setSuccessSkills([]);
    setErrorMsg(null);
    const formData = new FormData();

    const isSingleArchive =
      files.length === 1 &&
      (files[0].name.endsWith('.zip') || files[0].name.endsWith('.7z'));

    if (isSingleArchive) {
      formData.append('file', files[0]);
    } else {
      for (const file of files) {
        const name = file.name;
        if (name === '.DS_Store' || name === 'Thumbs.db') continue;

        const path = file.relativePath || file.webkitRelativePath || file.name;
        formData.append('files', file, path);
      }
    }

    try {
      const res = await fetch(apiUrl('/review/skills/upload'), {
        method: 'POST',
        body: formData,
      });

      if (res.ok) {
        try {
          const data = await res.json();
          setSuccessSkills(data);
        } catch (e) {
          console.error(e);
        }
        fetchSkills();
        onRefreshSkills();
        if (fileInputRef.current) fileInputRef.current.value = '';
        if (folderInputRef.current) folderInputRef.current.value = '';
      } else {
        const text = await res.text();
        setErrorMsg(text || '上传失败，请检查压缩包或文件夹是否符合 Claude 技能规范');
      }
    } catch (e) {
      setErrorMsg('上传发生网络异常，请重试');
      console.error(e);
    } finally {
      setUploading(false);
    }
  };

  const importPaths = async (paths: string[]) => {
    if (paths.length === 0) return;

    setUploading(true);
    setSuccessSkills([]);
    setErrorMsg(null);

    try {
      const res = await fetch(apiUrl('/review/skills/import'), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ paths }),
      });

      if (res.ok) {
        try {
          const data = await res.json();
          setSuccessSkills(data);
        } catch (e) {
          console.error(e);
        }
        fetchSkills();
        onRefreshSkills();
      } else {
        const text = await res.text();
        setErrorMsg(text || '导入失败，请检查文件/文件夹是否符合 Claude 技能规范');
      }
    } catch (e) {
      setErrorMsg('导入发生网络异常，请重试');
      console.error(e);
    } finally {
      setUploading(false);
    }
  };

  const handleDelete = async (id: string, name: string) => {
    if (!window.confirm(`确定要删除技能「${name}」吗？`)) {
      return;
    }

    setErrorMsg(null);
    try {
      const res = await fetch(apiUrl(`/review/skills/${id}`), {
        method: 'DELETE',
      });

      if (res.ok) {
        fetchSkills();
        onRefreshSkills();
      } else {
        const text = await res.text();
        setErrorMsg(text || '删除失败');
      }
    } catch (e) {
      setErrorMsg('网络异常，删除失败');
      console.error(e);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center transition-all duration-300"
      style={{ backgroundColor: 'rgba(255, 255, 255, 0.45)', backdropFilter: 'blur(16px)' }}
    >
      <div
        className="relative w-full max-w-2xl overflow-hidden rounded-[24px] p-6 text-slate-800 flex flex-col max-h-[85vh] transition-all"
        style={{
          background: '#ffffff',
          boxShadow: '0 20px 50px rgba(0, 0, 0, 0.05), 0 4px 12px rgba(0, 0, 0, 0.02)',
          border: '1px solid #e2e8f0',
        }}
      >
        
        {/* Header */}
        <div className="flex items-center justify-between pb-4 border-b border-slate-100">
          <h2 className="text-lg font-bold bg-gradient-to-r from-teal-500 to-emerald-600 bg-clip-text text-transparent flex items-center gap-2">
            <span>⚙️ 诊断技能管理器</span>
          </h2>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-xl bg-slate-100 hover:bg-slate-200 flex items-center justify-center transition-colors cursor-pointer text-slate-500 hover:text-slate-700"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Content Body */}
        <div className="flex-1 overflow-y-auto py-4 px-2 space-y-4">
          {errorMsg && (
            <div className="bg-rose-50 border border-rose-200 text-rose-600 rounded-xl p-3 text-sm flex items-center justify-between">
              <span>⚠️ {errorMsg}</span>
              <button onClick={() => setErrorMsg(null)} className="text-rose-500 hover:text-rose-700">
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          )}

          {/* Upload Area */}
          <div
            onDragEnter={handleDrag}
            onDragOver={handleDrag}
            onDragLeave={handleDrag}
            onDrop={handleDrop}
            className={`relative border-2 border-dashed rounded-2xl p-6 flex flex-col items-center justify-center gap-2 transition-all duration-300 ${
              dragActive
                ? 'border-teal-500 bg-teal-50/60 scale-[1.01] shadow-inner shadow-teal-500/5 ring-4 ring-teal-500/10'
                : 'border-slate-200 hover:border-slate-300 bg-slate-50/50 hover:bg-slate-50'
            }`}
          >
            {dragActive && (
              <div className="absolute inset-0 bg-transparent z-10" />
            )}
            <input
              type="file"
              ref={fileInputRef}
              onChange={handleFileChange}
              accept=".zip,.7z"
              className="hidden"
            />
            <input
              type="file"
              ref={folderInputRef}
              onChange={handleFolderChange}
              {...{ webkitdirectory: "", directory: "", multiple: true }}
              className="hidden"
            />
            {uploading ? (
              <div className="flex flex-col items-center gap-2">
                <div className="w-8 h-8 border-2 border-teal-500 border-t-transparent rounded-full animate-spin" />
                <span className="text-sm text-teal-600 font-medium">正在上传解析技能包...</span>
              </div>
            ) : (
              <>
                <svg className="w-10 h-10 text-slate-400 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
                <div className="text-center space-y-3">
                  <p className="text-sm font-medium text-slate-700">
                    拖拽 `.zip` / `.7z` 压缩包或整个技能文件夹至此
                  </p>
                  <div className="flex gap-4 justify-center">
                    <button
                      type="button"
                      onClick={() => fileInputRef.current?.click()}
                      className="px-3.5 py-2 bg-teal-500 hover:bg-teal-600 text-white rounded-xl text-xs font-semibold shadow transition-all duration-200 cursor-pointer"
                    >
                      选择压缩文件
                    </button>
                    <button
                      type="button"
                      onClick={() => folderInputRef.current?.click()}
                      className="px-3.5 py-2 bg-white hover:bg-slate-50 text-slate-700 rounded-xl text-xs font-semibold border border-slate-200 shadow-sm transition-all duration-200 cursor-pointer"
                    >
                      选择技能文件夹
                    </button>
                  </div>
                  <p className="text-xs text-slate-500 mt-1">
                    压缩包或文件夹内需包含包含 `SKILL.md` 的文件夹，符合 Claude Skills 规范
                  </p>
                </div>
              </>
            )}
          </div>

          {/* Upload Success Alert */}
          {successSkills.length > 0 && (
            <div
              className="p-4 rounded-2xl border flex flex-col gap-3 relative transition-all animate-fade-in"
              style={{
                borderColor: 'rgba(16, 185, 129, 0.25)',
                background: 'linear-gradient(135deg, rgba(16, 185, 129, 0.08) 0%, rgba(16, 185, 129, 0.02) 100%)',
              }}
            >
              <div className="flex items-center justify-between">
                <span className="text-xs font-bold text-emerald-600 flex items-center gap-1.5">
                  <span className="text-sm">🎉</span> 成功上传并解析 {successSkills.length} 个诊断技能规范！
                </span>
                <button
                  onClick={() => setSuccessSkills([])}
                  className="w-5 h-5 rounded-lg bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-600 flex items-center justify-center transition-colors cursor-pointer text-xs border-none"
                >
                  ✕
                </button>
              </div>
              <div className="space-y-2 border-t border-emerald-500/10 pt-2">
                {successSkills.map((s) => (
                  <div key={s.id} className="text-left">
                    <p className="text-xs font-bold text-slate-800">{s.name}</p>
                    <p className="text-[11px] text-slate-600 mt-1 leading-relaxed" title={s.description}>
                      技能描述：{s.description
                        ? (s.description.length > 200 ? s.description.slice(0, 200) + '...' : s.description)
                        : '暂无描述信息'}
                    </p>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Skills List */}
          <div className="space-y-3">
            <h3 className="text-sm font-semibold text-slate-500 flex items-center gap-1.5">
              <span>📋 当前已检测到的技能 ({skills.length})</span>
            </h3>

            {loading ? (
              <div className="flex items-center justify-center py-8">
                <div className="w-6 h-6 border-2 border-slate-400 border-t-transparent rounded-full animate-spin" />
              </div>
            ) : skills.length === 0 ? (
              <div className="text-center py-8 text-sm text-slate-500 border border-slate-100 rounded-2xl bg-slate-50/50">
                暂无技能，请上传压缩包，或在全局配置的 `skills/default` 目录放置默认规范
              </div>
            ) : (
              <div className="grid grid-cols-1 gap-3">
                {skills.map((skill) => (
                  <div
                    key={skill.id}
                    className="flex items-start justify-between p-4 rounded-2xl border border-slate-100 bg-slate-50/30 hover:bg-slate-50/80 transition-all duration-200"
                  >
                    <div className="space-y-1 pr-4">
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-slate-800">{skill.name}</span>
                        {skill.is_builtin ? (
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-teal-50 text-teal-600 border border-teal-200 font-medium">
                            内置
                          </span>
                        ) : (
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-50 text-amber-600 border border-amber-200 font-medium">
                            自定义
                          </span>
                        )}
                      </div>
                      <p className="text-xs text-slate-600 leading-relaxed" title={skill.description}>
                        {skill.description
                          ? (skill.description.length > 200 ? skill.description.slice(0, 200) + '...' : skill.description)
                          : '暂无描述信息'}
                      </p>
                      <p className="text-[10px] text-slate-400 font-mono">ID: {skill.id}</p>
                    </div>

                    {!skill.is_builtin && (
                      <button
                        onClick={() => handleDelete(skill.id, skill.name)}
                        className="rounded-xl p-1.5 hover:bg-rose-50 text-slate-400 hover:text-rose-600 border border-transparent hover:border-rose-200 transition-all cursor-pointer"
                        title="删除技能"
                      >
                        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="pt-4 border-t border-slate-100 flex justify-end">
          <button
            onClick={onClose}
            className="px-5 py-2.5 rounded-xl border border-slate-200 bg-white hover:bg-slate-50 text-slate-600 hover:text-slate-800 text-sm font-semibold transition-all cursor-pointer shadow-sm"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
