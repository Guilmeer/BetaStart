import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import type {
  MemoryFile,
  MemoryHit,
  NewTask,
  NoteDetail,
  ProjectInfo,
  Task,
} from "./types";

const STATUSES = ["todo", "in_progress", "done"] as const;
const STATUS_LABEL: Record<string, string> = {
  todo: "A fazer",
  in_progress: "Fazendo",
  done: "Feito",
  blocked: "Bloqueado",
  cancelled: "Cancelado",
};
const PRIORITY_LABEL: Record<string, string> = {
  high: "Alta",
  medium: "Média",
  low: "Baixa",
};

function toLocalInput(iso: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso.slice(0, 16);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function formatEpoch(timestamp: number | null): string {
  if (!timestamp) return "";
  return new Date(timestamp * 1000).toLocaleString();
}

function kindIcon(kind: string): string {
  if (kind === "task") return "☑";
  if (kind === "project") return "▣";
  return "▤";
}

export function Memory({ onBack }: { onBack: () => void }) {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [files, setFiles] = useState<MemoryFile[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [selectedProject, setSelectedProject] = useState("all");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [note, setNote] = useState<NoteDetail | null>(null);
  const [search, setSearch] = useState("");
  const [searchHits, setSearchHits] = useState<MemoryHit[] | null>(null);
  const [view, setView] = useState<"list" | "kanban">("list");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [newProject, setNewProject] = useState("");
  const [taskTitle, setTaskTitle] = useState("");
  const [taskDue, setTaskDue] = useState("");
  const [taskPriority, setTaskPriority] = useState("medium");
  const [taskParent, setTaskParent] = useState("");
  const [taskTags, setTaskTags] = useState("");

  const projectFilter = selectedProject === "all" ? null : selectedProject;

  const refresh = useCallback(async () => {
    try {
      const [projs, fs, ts] = await Promise.all([
        api.listProjects(),
        api.listMemoryFiles(projectFilter),
        api.listTasks(projectFilter, null, null),
      ]);
      setProjects(projs);
      setFiles(fs);
      setTasks(ts);
    } catch (error) {
      setMessage(String(error));
    }
  }, [projectFilter]);

  useEffect(() => {
    let cancelled = false;
    setBusy(true);
    api
      .reindexMemory()
      .then(() => {
        if (!cancelled) return refresh();
      })
      .catch((error) => setMessage(String(error)))
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!search.trim()) {
      setSearchHits(null);
      return;
    }
    const timer = window.setTimeout(() => {
      api
        .searchMemory(projectFilter, search, [], 50)
        .then(setSearchHits)
        .catch((error) => setMessage(String(error)));
    }, 120);
    return () => window.clearTimeout(timer);
  }, [search, projectFilter]);

  const openFile = useCallback(async (path: string) => {
    try {
      const detail = await api.readNote(path);
      setSelectedPath(path);
      setNote(detail);
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  const updateNote = (patch: Partial<NoteDetail>) =>
    setNote((current) => (current ? { ...current, ...patch } : current));

  const saveNote = useCallback(async () => {
    if (!note) return;
    setBusy(true);
    setMessage(null);
    try {
      if (note.kind === "task") {
        await api.updateTask(note.id ?? "", {
          title: note.title,
          status: note.status ?? "todo",
          priority: note.priority ?? "medium",
          due: note.due ?? "",
          remind: note.remind ?? "",
          tags: note.tags,
          body: note.body,
          parent: note.parent ?? "",
        });
      } else {
        await api.saveNote(note.path, note.title, note.tags, note.body);
      }
      await refresh();
      setNote(await api.readNote(note.path));
      setMessage("Salvo.");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [note, refresh]);

  const createNote = useCallback(async () => {
    setBusy(true);
    try {
      const created = await api.createNote(projectFilter, "Nova nota");
      setSelectedPath(created.path);
      setNote(created);
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [projectFilter, refresh]);

  const deleteNote = useCallback(async () => {
    if (!note) return;
    setBusy(true);
    try {
      await api.deleteMemoryFile(note.path);
      setNote(null);
      setSelectedPath(null);
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [note, refresh]);

  const createProject = useCallback(async () => {
    const name = newProject.trim();
    if (!name) return;
    setBusy(true);
    try {
      await api.createProject(name, name);
      setNewProject("");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [newProject, refresh]);

  const createTask = useCallback(async () => {
    const title = taskTitle.trim();
    if (!title) return;
    setBusy(true);
    try {
      const input: NewTask = {
        title,
        project: projectFilter,
        parent: taskParent || null,
        due: taskDue || null,
        priority: taskPriority,
        tags: taskTags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
      };
      await api.createTask(input);
      setTaskTitle("");
      setTaskDue("");
      setTaskTags("");
      setTaskParent("");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [
    taskTitle,
    projectFilter,
    taskParent,
    taskDue,
    taskPriority,
    taskTags,
    refresh,
  ]);

  const setStatus = useCallback(
    async (task: Task, status: string) => {
      setBusy(true);
      try {
        await api.updateTask(task.id, { status });
        await refresh();
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const removeTask = useCallback(
    async (task: Task) => {
      setBusy(true);
      try {
        await api.deleteTask(task.id);
        if (selectedPath === task.filePath) {
          setNote(null);
          setSelectedPath(null);
        }
        await refresh();
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [refresh, selectedPath],
  );

  const grouped = useMemo(() => {
    const map: Record<string, MemoryFile[]> = {};
    for (const file of files) {
      (map[file.project] ??= []).push(file);
    }
    return map;
  }, [files]);

  const roots = useMemo(() => tasks.filter((task) => !task.parentId), [tasks]);
  const childrenOf = (id: string) => tasks.filter((task) => task.parentId === id);

  const renderTask = (task: Task) => (
    <div className={`task-card ${task.status === "done" ? "done" : ""}`} key={task.id}>
      <div className="task-card-head">
        <strong onClick={() => void openFile(task.filePath)}>{task.title}</strong>
        <span className={`prio prio-${task.priority}`}>
          {PRIORITY_LABEL[task.priority] ?? task.priority}
        </span>
      </div>
      {(task.due || task.tags.length > 0) && (
        <div className="task-meta">
          {task.due && <span>⏰ {formatEpoch(task.due)}</span>}
          {task.tags.map((tag) => (
            <span className="tag" key={tag}>
              {tag}
            </span>
          ))}
        </div>
      )}
      <div className="task-actions">
        {STATUSES.map((status) => (
          <button
            key={status}
            className={task.status === status ? "active" : ""}
            disabled={busy}
            onClick={() => void setStatus(task, status)}
          >
            {STATUS_LABEL[status]}
          </button>
        ))}
        <button disabled={busy} onClick={() => void removeTask(task)}>
          ×
        </button>
      </div>
    </div>
  );

  return (
    <section className="memory">
      <div className="mem-header">
        <h1>Memória & Tasks</h1>
        <div className="inline">
          <button className="secondary" onClick={onBack}>
            Voltar
          </button>
          <button className="secondary" disabled={busy} onClick={() => void refresh()}>
            Atualizar
          </button>
        </div>
      </div>

      <div className="mem-toolbar">
        <input
          className="text-input"
          placeholder="Buscar na memória…"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        <button className="primary" disabled={busy} onClick={() => void createNote()}>
          Nova nota
        </button>
      </div>

      <div className="mem-body">
        <aside className="mem-sidebar">
          <div className="mem-projects">
            <h2>Projetos</h2>
            <button
              className={selectedProject === "all" ? "active" : ""}
              onClick={() => setSelectedProject("all")}
            >
              Todos
            </button>
            <button
              className={selectedProject === "global" ? "active" : ""}
              onClick={() => setSelectedProject("global")}
            >
              Global
            </button>
            {projects.map((project) => (
              <button
                key={project.slug}
                className={selectedProject === project.slug ? "active" : ""}
                onClick={() => setSelectedProject(project.slug)}
              >
                {project.title}
              </button>
            ))}
            <div className="inline">
              <input
                className="text-input"
                placeholder="Novo projeto"
                value={newProject}
                onChange={(event) => setNewProject(event.target.value)}
              />
              <button
                className="secondary"
                disabled={busy || !newProject.trim()}
                onClick={() => void createProject()}
              >
                +
              </button>
            </div>
          </div>

          <div className="mem-files">
            <h2>{searchHits ? "Busca" : "Arquivos"}</h2>
            {searchHits ? (
              searchHits.map((hit) => (
                <button
                  key={hit.filePath}
                  className="mem-file"
                  onClick={() => void openFile(hit.filePath)}
                >
                  <span className={`kind kind-${hit.kind}`}>{kindIcon(hit.kind)}</span>
                  <span className="mem-file-text">
                    <strong>{hit.title}</strong>
                    <small>
                      {hit.project} · {hit.kind}
                    </small>
                  </span>
                </button>
              ))
            ) : (
              Object.entries(grouped).map(([project, list]) => (
                <div key={project}>
                  <div className="mem-group">{project === "global" ? "Global" : project}</div>
                  {list.map((file) => (
                    <button
                      key={file.path}
                      className={`mem-file ${selectedPath === file.path ? "active" : ""}`}
                      onClick={() => void openFile(file.path)}
                    >
                      <span className={`kind kind-${file.kind}`}>{kindIcon(file.kind)}</span>
                      <span className="mem-file-text">
                        <strong>{file.title}</strong>
                        <small>{file.kind}</small>
                      </span>
                    </button>
                  ))}
                </div>
              ))
            )}
          </div>
        </aside>

        <main className="mem-editor">
          {note ? (
            <>
              <div className="mem-editor-head">
                <input
                  className="note-title"
                  value={note.title}
                  onChange={(event) => updateNote({ title: event.target.value })}
                />
                <div className="mem-editor-actions">
                  <button className="primary" disabled={busy} onClick={() => void saveNote()}>
                    Salvar
                  </button>
                  <button className="secondary" disabled={busy} onClick={() => void deleteNote()}>
                    Excluir
                  </button>
                </div>
              </div>

              <div className="note-meta">
                <span className="kind-badge">{note.kind}</span>
                {note.project && <span className="kind-badge">{note.project}</span>}
                {note.id && <span className="kind-badge">#{note.id.slice(0, 8)}</span>}
              </div>

              {note.kind === "task" && (
                <div className="task-fields">
                  <select
                    className="text-input"
                    value={note.status ?? "todo"}
                    onChange={(event) => updateNote({ status: event.target.value })}
                  >
                    {STATUSES.map((status) => (
                      <option key={status} value={status}>
                        {STATUS_LABEL[status]}
                      </option>
                    ))}
                  </select>
                  <select
                    className="text-input"
                    value={note.priority ?? "medium"}
                    onChange={(event) => updateNote({ priority: event.target.value })}
                  >
                    {Object.entries(PRIORITY_LABEL).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
                      </option>
                    ))}
                  </select>
                  <input
                    className="text-input"
                    type="datetime-local"
                    title="Data de vencimento"
                    value={toLocalInput(note.due)}
                    onChange={(event) => updateNote({ due: event.target.value })}
                  />
                  <input
                    className="text-input"
                    type="datetime-local"
                    title="Lembrete"
                    value={toLocalInput(note.remind)}
                    onChange={(event) => updateNote({ remind: event.target.value })}
                  />
                </div>
              )}

              <input
                className="text-input"
                placeholder="tags, separadas por vírgula"
                value={note.tags.join(", ")}
                onChange={(event) =>
                  updateNote({
                    tags: event.target.value
                      .split(",")
                      .map((tag) => tag.trim())
                      .filter(Boolean),
                  })
                }
              />

              <textarea
                className="note-body"
                value={note.body}
                onChange={(event) => updateNote({ body: event.target.value })}
                placeholder="Escreva aqui…"
              />
            </>
          ) : (
            <div className="mem-empty">Selecione um arquivo ou crie uma nota.</div>
          )}
          {message && <p className="settings-msg">{message}</p>}
        </main>

        <aside className="mem-tasks">
          <div className="mem-tasks-head">
            <h2>Tasks</h2>
            <div className="segmented">
              <button
                className={view === "list" ? "active" : ""}
                onClick={() => setView("list")}
              >
                Lista
              </button>
              <button
                className={view === "kanban" ? "active" : ""}
                onClick={() => setView("kanban")}
              >
                Kanban
              </button>
            </div>
          </div>

          <div className="task-form">
            <input
              className="text-input"
              placeholder="Nova task…"
              value={taskTitle}
              onChange={(event) => setTaskTitle(event.target.value)}
            />
            <div className="inline">
              <select
                className="text-input"
                value={taskPriority}
                onChange={(event) => setTaskPriority(event.target.value)}
              >
                <option value="high">Alta</option>
                <option value="medium">Média</option>
                <option value="low">Baixa</option>
              </select>
              <input
                className="text-input"
                type="datetime-local"
                value={taskDue}
                onChange={(event) => setTaskDue(event.target.value)}
              />
            </div>
            <div className="inline">
              <input
                className="text-input"
                placeholder="tags, vírgula"
                value={taskTags}
                onChange={(event) => setTaskTags(event.target.value)}
              />
              <select
                className="text-input"
                value={taskParent}
                onChange={(event) => setTaskParent(event.target.value)}
              >
                <option value="">Sem pai</option>
                {roots.map((task) => (
                  <option key={task.id} value={task.id}>
                    {task.title}
                  </option>
                ))}
              </select>
            </div>
            <button
              className="primary"
              disabled={busy || !taskTitle.trim()}
              onClick={() => void createTask()}
            >
              Adicionar task
            </button>
          </div>

          {view === "list" ? (
            <div className="task-list">
              {STATUSES.map((status) => {
                const list = tasks.filter(
                  (task) => task.status === status && !task.parentId,
                );
                if (list.length === 0) return null;
                return (
                  <div className="task-group" key={status}>
                    <div className="task-group-title">{STATUS_LABEL[status]}</div>
                    {list.map((task) => (
                      <div key={task.id}>
                        {renderTask(task)}
                        {childrenOf(task.id).map((child) => (
                          <div className="subtask" key={child.id}>
                            {renderTask(child)}
                          </div>
                        ))}
                      </div>
                    ))}
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="kanban">
              {STATUSES.map((status) => (
                <div className="kanban-col" key={status}>
                  <div className="kanban-title">{STATUS_LABEL[status]}</div>
                  {tasks
                    .filter((task) => task.status === status)
                    .map((task) => (
                      <div className="task-card kanban-card" key={task.id}>
                        <strong onClick={() => void openFile(task.filePath)}>
                          {task.parentId ? "↳ " : ""}
                          {task.title}
                        </strong>
                        <div className="task-actions">
                          {STATUSES.filter((item) => item !== status).map((item) => (
                            <button
                              key={item}
                              disabled={busy}
                              onClick={() => void setStatus(task, item)}
                            >
                              → {STATUS_LABEL[item]}
                            </button>
                          ))}
                        </div>
                      </div>
                    ))}
                </div>
              ))}
            </div>
          )}
        </aside>
      </div>
    </section>
  );
}
