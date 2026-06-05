import React, { useState, useEffect, useRef } from 'react';
import { apiUrl } from '../lib/api';

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
  const fileInputRef = useRef<HTMLInputElement>(null);

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
    if (isOpen) {
      fetchSkills();
    }
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

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(false);
    setErrorMsg(null);

    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      const file = e.dataTransfer.files[0];
      await uploadFile(file);
    }
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    setErrorMsg(null);
    if (e.target.files && e.target.files[0]) {
      const file = e.target.files[0];
      await uploadFile(file);
    }
  };

  const uploadFile = async (file: File) => {
    const isZip = file.name.endsWith('.zip');
    const is7z = file.name.endsWith('.7z');
    if (!isZip && !is7z) {
      setErrorMsg('仅支持上传 .zip 或 .7z 压缩包');
      return;
    }

    setUploading(true);
    const formData = new FormData();
    formData.append('file', file);

    try {
      const res = await fetch(apiUrl('/review/skills/upload'), {
        method: 'POST',
        body: formData,
      });

      if (res.ok) {
        fetchSkills();
        onRefreshSkills();
        if (fileInputRef.current) fileInputRef.current.value = '';
      } else {
        const text = await res.text();
        setErrorMsg(text || '上传失败，请检查压缩包是否符合 Claude 技能规范');
      }
    } catch (e) {
      setErrorMsg('上传发生网络异常，请重试');
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-md transition-all duration-300">
      <div className="relative w-full max-w-2xl overflow-hidden rounded-[24px] border border-white/10 bg-zinc-900/90 shadow-2xl p-6 text-zinc-100 flex flex-col max-h-[85vh]">
        
        {/* Header */}
        <div className="flex items-center justify-between pb-4 border-b border-white/5">
          <h2 className="text-xl font-bold bg-gradient-to-r from-teal-400 to-emerald-400 bg-clip-text text-transparent flex items-center gap-2">
            <span>⚙️ 诊断技能管理器</span>
          </h2>
          <button
            onClick={onClose}
            className="rounded-full p-1.5 hover:bg-white/10 text-zinc-400 hover:text-zinc-100 transition-colors"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Content Body */}
        <div className="flex-1 overflow-y-auto py-4 space-y-4 pr-1">
          {errorMsg && (
            <div className="bg-rose-500/10 border border-rose-500/20 text-rose-400 rounded-xl p-3 text-sm flex items-center justify-between">
              <span>⚠️ {errorMsg}</span>
              <button onClick={() => setErrorMsg(null)} className="text-rose-400 hover:text-rose-200">
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
            onClick={() => fileInputRef.current?.click()}
            className={`border-2 border-dashed rounded-2xl p-6 flex flex-col items-center justify-center gap-2 cursor-pointer transition-all duration-300 ${
              dragActive
                ? 'border-teal-500 bg-teal-500/5'
                : 'border-white/10 hover:border-white/20 bg-white/[0.02] hover:bg-white/[0.04]'
            }`}
          >
            <input
              type="file"
              ref={fileInputRef}
              onChange={handleFileChange}
              accept=".zip,.7z"
              className="hidden"
            />
            {uploading ? (
              <div className="flex flex-col items-center gap-2">
                <div className="w-8 h-8 border-2 border-teal-500 border-t-transparent rounded-full animate-spin" />
                <span className="text-sm text-teal-400 font-medium">正在解析解压技能包...</span>
              </div>
            ) : (
              <>
                <svg className="w-10 h-10 text-zinc-400 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
                <div className="text-center">
                  <p className="text-sm font-medium text-zinc-200">
                    点击选择 或 将 `.zip` / `.7z` 压缩包拖拽至此
                  </p>
                  <p className="text-xs text-zinc-500 mt-1">
                    压缩包内需包含含有 `SKILL.md` 的文件夹，符合 Claude Skills 规范
                  </p>
                </div>
              </>
            )}
          </div>

          {/* Skills List */}
          <div className="space-y-3">
            <h3 className="text-sm font-semibold text-zinc-400 flex items-center gap-1.5">
              <span>📋 当前已检测到的技能 ({skills.length})</span>
            </h3>

            {loading ? (
              <div className="flex items-center justify-center py-8">
                <div className="w-6 h-6 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
              </div>
            ) : skills.length === 0 ? (
              <div className="text-center py-8 text-sm text-zinc-500 border border-white/5 rounded-2xl bg-white/[0.01]">
                暂无技能，请上传压缩包或在项目 `.agents/skills` 放置内置规范
              </div>
            ) : (
              <div className="grid grid-cols-1 gap-3">
                {skills.map((skill) => (
                  <div
                    key={skill.id}
                    className="flex items-start justify-between p-4 rounded-2xl border border-white/5 bg-white/[0.02] hover:bg-white/[0.04] transition-all duration-200"
                  >
                    <div className="space-y-1 pr-4">
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-zinc-200">{skill.name}</span>
                        {skill.is_builtin ? (
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-teal-500/10 text-teal-400 border border-teal-500/20 font-medium">
                            内置
                          </span>
                        ) : (
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20 font-medium">
                            自定义
                          </span>
                        )}
                      </div>
                      <p className="text-xs text-zinc-400 leading-relaxed">
                        {skill.description || '暂无描述信息'}
                      </p>
                      <p className="text-[10px] text-zinc-600 font-mono">ID: {skill.id}</p>
                    </div>

                    {!skill.is_builtin && (
                      <button
                        onClick={() => handleDelete(skill.id, skill.name)}
                        className="rounded-xl p-1.5 hover:bg-rose-500/10 text-zinc-500 hover:text-rose-400 transition-all cursor-pointer"
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
        <div className="pt-4 border-t border-white/5 flex justify-end">
          <button
            onClick={onClose}
            className="px-5 py-2.5 rounded-xl bg-white/5 hover:bg-white/10 text-zinc-300 hover:text-zinc-100 text-sm font-medium transition-all cursor-pointer"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
