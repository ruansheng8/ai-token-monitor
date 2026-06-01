import { useEffect, useState, useCallback } from 'react';
import { apiUrl, readJsonResponse } from '../lib/api';
import { AlertCircle, Plus, Trash2, Copy, Check, X, ShieldAlert } from 'lucide-react';

export interface PromptTemplate {
  id: string;
  name: string;
  description: string | null;
  template: string;
  is_builtin: number;
  created_at: string;
  updated_at: string;
}

interface PromptTemplateManagerModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectTemplate: (templateId: string, templateContent: string) => void;
}

export function PromptTemplateManagerModal({ isOpen, onClose, onSelectTemplate }: PromptTemplateManagerModalProps) {
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  // 选中的模板，如果为 'new' 则表示正在新建
  const [selectedId, setSelectedId] = useState<string | 'new' | null>(null);
  
  // 编辑表单状态
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formTemplate, setFormTemplate] = useState('');
  
  // 是否正在保存/删除中
  const [actionLoading, setActionLoading] = useState(false);
  
  // 二次确认删除的 ID
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  // 辅助设定选中表单
  const selectTemplate = useCallback((tpl: PromptTemplate) => {
    setSelectedId(tpl.id);
    setFormName(tpl.name);
    setFormDescription(tpl.description || '');
    setFormTemplate(tpl.template);
    setConfirmDeleteId(null);
  }, []);

  // 1. 获取模板列表
  const fetchTemplates = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(apiUrl('/review/prompt_templates'));
      if (!res.ok) {
        throw new Error('获取提示词模板列表失败');
      }
      const data = await readJsonResponse<PromptTemplate[]>(res);
      setTemplates(data);
      
      // 默认选中第一个
      if (data.length > 0 && selectedId === null) {
        selectTemplate(data[0]);
      } else if (selectedId && selectedId !== 'new') {
        const found = data.find(t => t.id === selectedId);
        if (found) {
          selectTemplate(found);
        } else if (data.length > 0) {
          selectTemplate(data[0]);
        }
      }
    } catch (err: unknown) {
      const errMsg = err instanceof Error ? err.message : String(err);
      setError(errMsg);
    } finally {
      setLoading(false);
    }
  }, [selectedId, selectTemplate]);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    if (isOpen) {
      // 异步执行，规避 React 渲染周期内同步 setState (set-state-in-effect) 的规则
      timer = setTimeout(() => {
        fetchTemplates();
      }, 0);
    }
    return () => {
      if (timer) {
        clearTimeout(timer);
      }
    };
  }, [isOpen, fetchTemplates]);

  const handleSelectNew = () => {
    setSelectedId('new');
    setFormName('');
    setFormDescription('');
    setFormTemplate('');
    setConfirmDeleteId(null);
  };

  const handleClone = (sourceTpl: PromptTemplate) => {
    setSelectedId('new');
    setFormName(`${sourceTpl.name} (副本)`);
    setFormDescription(sourceTpl.description || '');
    setFormTemplate(sourceTpl.template);
    setConfirmDeleteId(null);
  };

  // 2. 保存/修改模板
  const handleSave = async () => {
    if (!formName.trim() || !formTemplate.trim()) {
      alert('模板名称和内容不能为空');
      return;
    }

    setActionLoading(true);
    try {
      if (selectedId === 'new') {
        // 创建
        const res = await fetch(apiUrl('/review/prompt_templates'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            name: formName.trim(),
            description: formDescription.trim() || null,
            template: formTemplate.trim(),
          }),
        });
        const data = await readJsonResponse<{ success: boolean; id?: string; message?: string }>(res);
        if (res.ok && data.success && data.id) {
          setSelectedId(data.id);
          await fetchTemplates();
        } else {
          throw new Error(data.message || '新建模板失败');
        }
      } else {
        // 更新
        const res = await fetch(apiUrl(`/review/prompt_templates/${selectedId}`), {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            name: formName.trim(),
            description: formDescription.trim() || null,
            template: formTemplate.trim(),
          }),
        });
        const data = await readJsonResponse<{ success: boolean; message?: string }>(res);
        if (res.ok && data.success) {
          await fetchTemplates();
        } else {
          throw new Error(data.message || '更新模板失败');
        }
      }
    } catch (err: unknown) {
      const errMsg = err instanceof Error ? err.message : String(err);
      alert(errMsg || '保存失败');
    } finally {
      setActionLoading(false);
    }
  };

  // 3. 删除模板
  const handleDelete = async (id: string) => {
    setActionLoading(true);
    try {
      const res = await fetch(apiUrl(`/review/prompt_templates/${id}`), {
        method: 'DELETE',
      });
      const data = await readJsonResponse<{ success: boolean; message?: string }>(res);
      if (res.ok && data.success) {
        setSelectedId(null);
        setConfirmDeleteId(null);
        await fetchTemplates();
      } else {
        throw new Error(data.message || '删除模板失败');
      }
    } catch (err: unknown) {
      const errMsg = err instanceof Error ? err.message : String(err);
      alert(errMsg || '删除失败');
    } finally {
      setActionLoading(false);
    }
  };

  // 4. 应用模板并关闭弹窗
  const handleUseTemplate = (tpl: PromptTemplate) => {
    onSelectTemplate(tpl.id, tpl.template);
    onClose();
  };

  if (!isOpen) return null;

  const currentSelectedTpl = templates.find(t => t.id === selectedId);
  const isBuiltin = currentSelectedTpl?.is_builtin === 1;

  return (
    <>
      {/* 遮罩 */}
      <div
        className="fixed inset-0 bg-slate-900/40 backdrop-blur-[2px] z-[999] transition-opacity duration-300"
        onClick={onClose}
      />

      {/* 居中模态弹窗 - 亮色玻璃拟态风格 */}
      <div
        className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[850px] max-w-[95vw] h-[580px] max-h-[90vh] bg-white/95 border border-slate-200/80 rounded-[24px] z-[1000] shadow-[0_24px_64px_rgba(15,23,42,0.15)] flex flex-col text-left overflow-hidden transition-all duration-300"
      >
        {/* 头部 */}
        <div className="p-4 border-b border-slate-200/80 flex justify-between items-center bg-slate-50/80">
          <div>
            <h3 className="text-sm font-semibold text-slate-800 flex items-center gap-2">
              <span className="w-2.5 h-2.5 rounded-full bg-cyan-500 animate-pulse shadow-[0_0_8px_#06b6d4]"></span>
              专家分析提示词模板管理
            </h3>
            <p className="text-[10px] text-slate-500 mt-0.5">
              自定义复盘提问视角，指导 AI 专家进行深度用量、模式与成本诊断
            </p>
          </div>
          <button
            onClick={onClose}
            className="w-7 h-7 rounded-lg hover:bg-slate-200/60 text-slate-400 hover:text-slate-800 flex items-center justify-center transition-colors"
          >
            <X size={15} />
          </button>
        </div>

        {/* 主体区 */}
        <div className="flex-1 flex overflow-hidden">
          {/* 左侧列表 */}
          <div className="w-[300px] border-r border-slate-200/80 flex flex-col bg-slate-50/50">
            {/* 列表滚动内容 */}
            <div className="flex-1 overflow-y-auto p-3 space-y-2 scrollbar-thin">
              {loading && (
                <div className="h-40 flex flex-col items-center justify-center gap-2">
                  <div className="w-5 h-5 border-2 border-cyan-500 border-t-transparent rounded-full animate-spin"></div>
                  <p className="text-[11px] text-slate-400">载入列表中...</p>
                </div>
              )}

              {error && (
                <div className="p-3 rounded-lg border border-red-200 bg-red-50 text-center space-y-1">
                  <p className="text-[11px] text-red-500 font-semibold">⚠️ 载入失败</p>
                </div>
              )}

              {!loading && !error && (
                <>
                  {templates.map(tpl => {
                    const isSel = selectedId === tpl.id;
                    const builtIn = tpl.is_builtin === 1;
                    return (
                      <div
                        key={tpl.id}
                        onClick={() => selectTemplate(tpl)}
                        className={`p-3 rounded-xl border text-left cursor-pointer transition-all duration-150 relative overflow-hidden group ${
                          isSel
                            ? 'bg-cyan-50/80 border-cyan-300 text-slate-800 shadow-sm'
                            : 'bg-white/40 border-slate-100 text-slate-700 hover:bg-slate-100/50 hover:border-slate-200'
                        }`}
                      >
                        {isSel && (
                          <div className="absolute left-0 top-0 bottom-0 w-1 bg-cyan-500" />
                        )}
                        <div className="flex justify-between items-center gap-2">
                          <span className={`font-semibold text-xs truncate max-w-[160px] ${isSel ? 'text-cyan-800' : 'text-slate-800'}`}>
                            {tpl.name}
                          </span>
                          <span
                            className={`text-[9px] px-1.5 py-0.5 rounded font-mono scale-90 ${
                              builtIn
                                ? 'bg-blue-100 text-blue-700 border border-blue-200/50'
                                : 'bg-purple-100 text-purple-700 border border-purple-200/50'
                            }`}
                          >
                            {builtIn ? '系统' : '自定义'}
                          </span>
                        </div>
                        {tpl.description && (
                          <p className="text-[10px] text-slate-500 mt-1.5 truncate max-w-[240px]">
                            {tpl.description}
                          </p>
                        )}
                      </div>
                    );
                  })}
                </>
              )}
            </div>

            {/* 左侧底部添加按钮 */}
            <div className="p-3 border-t border-slate-200/80 bg-slate-50/80">
              <button
                onClick={handleSelectNew}
                className={`w-full py-2 px-3 rounded-xl border border-dashed flex items-center justify-center gap-1.5 text-xs font-medium transition-all ${
                  selectedId === 'new'
                    ? 'border-cyan-500 text-cyan-600 bg-cyan-50'
                    : 'border-slate-200 text-slate-500 hover:text-slate-800 hover:border-slate-300 hover:bg-white/60'
                }`}
              >
                <Plus size={14} />
                新建自定义模板
              </button>
            </div>
          </div>

          {/* 右侧编辑表单区 */}
          <div className="flex-1 flex flex-col p-4 overflow-hidden bg-white/40">
            <div className="flex-1 overflow-y-auto space-y-4 pr-1 scrollbar-thin">
              {selectedId === null ? (
                <div className="h-full flex flex-col items-center justify-center text-center text-slate-400 gap-2">
                  <AlertCircle size={24} className="text-slate-300" />
                  <p className="text-xs">请在左侧选择模板，或点击下方按钮新建模板</p>
                </div>
              ) : (
                <>
                  {/* 名称 */}
                  <div className="space-y-1">
                    <label className="text-[11px] font-bold text-slate-500 uppercase tracking-wider block">
                      模板名称 <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      value={formName}
                      onChange={e => setFormName(e.target.value)}
                      disabled={isBuiltin || actionLoading}
                      maxLength={40}
                      placeholder="例: 📊 成本节流专项"
                      className="w-full px-3 py-2 text-xs rounded-xl bg-white border border-slate-200 text-slate-800 placeholder-slate-400 focus:outline-none focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500/20 disabled:opacity-50 disabled:bg-slate-50 transition-all"
                    />
                  </div>

                  {/* 描述 */}
                  <div className="space-y-1">
                    <label className="text-[11px] font-bold text-slate-500 uppercase tracking-wider block">
                      简短描述
                    </label>
                    <input
                      type="text"
                      value={formDescription}
                      onChange={e => setFormDescription(e.target.value)}
                      disabled={isBuiltin || actionLoading}
                      maxLength={100}
                      placeholder="简述该模板的侧重点，便于切换模板时查看"
                      className="w-full px-3 py-2 text-xs rounded-xl bg-white border border-slate-200 text-slate-800 placeholder-slate-400 focus:outline-none focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500/20 disabled:opacity-50 disabled:bg-slate-50 transition-all"
                    />
                  </div>

                  {/* 提示词内容 */}
                  <div className="space-y-1 flex flex-col">
                    <label className="text-[11px] font-bold text-slate-500 uppercase tracking-wider block">
                      提示词模板内容 <span className="text-red-500">*</span>
                    </label>
                    <textarea
                      value={formTemplate}
                      onChange={e => setFormTemplate(e.target.value)}
                      disabled={isBuiltin || actionLoading}
                      placeholder="写下具体的分析指令。支持占位符如 {{IDE}}, {{TOTAL_TOKENS}}, {{TOTAL_COST}}, {{CACHE_HIT_RATE}} 等。"
                      className="w-full h-[200px] px-3 py-2 text-xs rounded-xl bg-white border border-slate-200 text-slate-800 placeholder-slate-400 focus:outline-none focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500/20 font-mono resize-none disabled:opacity-50 disabled:bg-slate-50 transition-all scrollbar-thin"
                    />
                    <div className="flex justify-between items-center text-[10px] text-slate-400 mt-1">
                      <span>支持内置自动替换占位符: <code className="bg-slate-100 px-1 py-0.5 rounded text-cyan-600 font-mono text-[9px] scale-95">&#123;&#123;IDE&#125;&#125;</code></span>
                      {isBuiltin && (
                        <span className="text-amber-600 flex items-center gap-0.5">
                          <ShieldAlert size={11} />
                          系统模板只读
                        </span>
                      )}
                    </div>
                  </div>
                </>
              )}
            </div>

            {/* 右侧底部操作按钮 */}
            {selectedId !== null && (
              <div className="mt-4 pt-3 border-t border-slate-200/85 flex justify-between items-center">
                {/* 左边：内置说明或自定义删除 */}
                <div>
                  {isBuiltin && currentSelectedTpl && (
                    <button
                      onClick={() => handleClone(currentSelectedTpl)}
                      className="py-1.5 px-3 rounded-lg border border-cyan-200 text-cyan-600 bg-cyan-50 hover:bg-cyan-100/60 text-[11px] font-medium transition-all flex items-center gap-1 cursor-pointer"
                    >
                      <Copy size={12} />
                      克隆为自定义副本
                    </button>
                  )}

                  {!isBuiltin && selectedId !== 'new' && (
                    <>
                      {confirmDeleteId === selectedId ? (
                        <div className="flex items-center gap-2">
                          <span className="text-[10px] text-red-500 font-semibold">确认删除该模板？</span>
                          <button
                            onClick={() => handleDelete(selectedId)}
                            className="bg-red-500 hover:bg-red-600 text-white text-[10px] py-1 px-2.5 rounded font-medium transition-all cursor-pointer"
                          >
                            确定
                          </button>
                          <button
                            onClick={() => setConfirmDeleteId(null)}
                            className="text-slate-500 hover:text-slate-800 text-[10px] py-1 px-2 transition-all cursor-pointer"
                          >
                            取消
                          </button>
                        </div>
                      ) : (
                        <button
                          onClick={() => setConfirmDeleteId(selectedId)}
                          disabled={actionLoading}
                          className="p-2 text-slate-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-all cursor-pointer"
                          title="删除模板"
                        >
                          <Trash2 size={15} />
                        </button>
                      )}
                    </>
                  )}
                </div>

                {/* 右边：应用或保存/取消 */}
                <div className="flex items-center gap-2">
                  {selectedId === 'new' ? (
                    <>
                      <button
                        onClick={() => {
                          if (templates.length > 0) {
                            selectTemplate(templates[0]);
                          } else {
                            setSelectedId(null);
                          }
                        }}
                        className="py-1.5 px-3 text-slate-500 hover:text-slate-800 text-[11px] font-medium transition-all cursor-pointer"
                      >
                        取消
                      </button>
                      <button
                        onClick={handleSave}
                        disabled={actionLoading}
                        className="py-1.5 px-4 bg-cyan-500 hover:bg-cyan-600 disabled:opacity-50 text-white rounded-xl text-[11px] font-bold shadow-sm transition-all flex items-center gap-1 cursor-pointer"
                      >
                        {actionLoading ? (
                          <div className="w-3.5 h-3.5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                        ) : (
                          <Check size={13} />
                        )}
                        创建模板
                      </button>
                    </>
                  ) : (
                    <>
                      {/* 如果不是新建，且选中的是自定义模板，有保存修改按钮 */}
                      {!isBuiltin && (
                        <button
                          onClick={handleSave}
                          disabled={actionLoading}
                          className="py-1.5 px-4 bg-purple-500 hover:bg-purple-600 disabled:opacity-50 text-white rounded-xl text-[11px] font-bold shadow-sm transition-all flex items-center gap-1 mr-2 cursor-pointer"
                        >
                          {actionLoading ? (
                            <div className="w-3.5 h-3.5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                          ) : (
                            <Check size={13} />
                          )}
                          保存修改
                        </button>
                      )}

                      {/* 应用本模板到输入框的按钮 */}
                      {currentSelectedTpl && (
                        <button
                          onClick={() => handleUseTemplate(currentSelectedTpl)}
                          className="py-1.5 px-4 bg-cyan-500 hover:bg-cyan-600 text-white rounded-xl text-[11px] font-bold shadow-sm transition-all cursor-pointer"
                        >
                          应用并选用该模板
                        </button>
                      )}
                    </>
                  )}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
