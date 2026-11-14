import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { Graph } from "./Graph";
import { useI18n } from "./i18n";
import { MarkdownInline, MarkdownView } from "./Markdown";
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
  todo: "To do",
  in_progress: "In progress",
  done: "Done",
  blocked: "Blocked",
  cancelled: "Cancelled",
};
const PRIORITY_LABEL: Record<string, string> = {
  high: "High",
  medium: "Medium",
  low: "Low",
};

function toLocalInput(iso: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso.slice(0, 16);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function formatEpoch(timestamp: number | null, locale: string): string {
  if (!timestamp) return "";
  return new Date(timestamp * 1000).toLocaleString(locale);
}

function formatIsoDisplay(iso: string | null, locale: string): string {
  if (!iso) return "";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString(locale);
}

function kindIcon(kind: string): string {
  if (kind === "task") return "☑";
  if (kind === "project") return "▣";
  return "▤";
}

function normalizePath(value: string): string {
  return value.replace(/\\/g, "/");
}

function joinPath(base: string, child: string): string {
  return `${normalizePath(base).replace(/\/+$/, "")}/${child}`;
}

function relativeToRoot(path: string, root: string | null | undefined): string {
  if (!root) return normalizePath(path);
  const value = normalizePath(path);
  const base = normalizePath(root).replace(/\/+$/, "");
  if (value.toLowerCase().startsWith(`${base.toLowerCase()}/`)) {
    return value.slice(base.length + 1);
  }
  return value;
}

function folderOf(path: string): string {
  const normalized = normalizePath(path);
  const index = normalized.lastIndexOf("/");
  return index > 0 ? normalized.slice(0, index) : "";
}

function folderLabel(folder: string): string {
  if (folder === ".betastart") return "Beta Start";
  if (folder.startsWith(".betastart/")) {
    return `Beta Start/${folder.slice(".betastart/".length)}`;
  }
  return folder;
}

function sameFilePath(left: string, right: string): boolean {
  return normalizePath(left).toLowerCase() === normalizePath(right).toLowerCase();
}

function resolveRelativePath(filePath: string, href: string): string {
  let target = href.split("#")[0].split("?")[0].trim();
  try {
    target = decodeURI(target);
  } catch {
    // mantém o valor original se o percent-encoding for inválido
  }
  const fileUrl = target.match(/^file:\/\/\/(.*)$/i);
  if (fileUrl) target = fileUrl[1];
  if (!target) return filePath;
  if (/^[a-zA-Z]:[\\/]/.test(target) || target.startsWith("/") || target.startsWith("\\")) {
    return target;
  }

  const separator = filePath.includes("\\") ? "\\" : "/";
  const base = filePath.slice(0, filePath.lastIndexOf(separator) + 1);
  const combined = `${base}${target}`;
  const normalizedSeparator = combined.includes("\\") ? "\\" : "/";
  const parts = combined
    .replace(/[\\/]/g, normalizedSeparator)
    .split(normalizedSeparator);
  const resolved: string[] = [];

  for (const part of parts) {
    if (part === "" && resolved.length === 0) {
      resolved.push(part);
      continue;
    }
    if (part === ".") continue;
    if (part === "..") {
      if (resolved.length > 1) resolved.pop();
      continue;
    }
    resolved.push(part);
  }

  return resolved.join(normalizedSeparator);
}

export function Memory({ onBack }: { onBack: () => void }) {
  const { t, locale } = useI18n();
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [files, setFiles] = useState<MemoryFile[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [selectedProject, setSelectedProject] = useState("all");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [note, setNote] = useState<NoteDetail | null>(null);
  const [noteDirty, setNoteDirty] = useState(false);
  const [backlinks, setBacklinks] = useState<MemoryFile[]>([]);
  const [workspacePath, setWorkspacePath] = useState("");
  const [search, setSearch] = useState("");
  const [searchHits, setSearchHits] = useState<MemoryHit[] | null>(null);
  const [view, setView] = useState<"list" | "kanban">("list");
  const [rightPanel, setRightPanel] = useState<"tasks" | "graph">("tasks");
  const [editorMode, setEditorMode] = useState<"read" | "edit">("read");
  const [expandedTasks, setExpandedTasks] = useState<Set<string>>(new Set());
  const [graphVersion, setGraphVersion] = useState(0);
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
      const [projs, fs, ts, paths] = await Promise.all([
        api.listProjects(),
        api.listMemoryFiles(projectFilter),
        api.listTasks(projectFilter, null, null),
        api.getPaths(),
      ]);
      setProjects(projs);
      setFiles(fs);
      setTasks(ts);
      setWorkspacePath(paths.workspace);
      setGraphVersion((value) => value + 1);
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
      setNoteDirty(false);
      setEditorMode("read");
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    let active = true;
    const path = note?.path;
    if (!path) {
      setBacklinks([]);
      return;
    }
    api
      .getBacklinks(path)
      .then((list) => {
        if (active) setBacklinks(list);
      })
      .catch(() => {
        if (active) setBacklinks([]);
      });
    return () => {
      active = false;
    };
  }, [note?.path, files]);

  const resolveWikiLink = useCallback(
    (label: string) => {
      const target = label.trim().toLowerCase();
      if (!target) return null;
      const fromNote = note?.links.find(
        (link) => link.label.trim().toLowerCase() === target,
      );
      if (fromNote?.path) return fromNote.path;
      const file = files.find((item) => {
        const title = item.title.trim().toLowerCase();
        const name = item.name.replace(/\.md$/i, "").trim().toLowerCase();
        return title === target || name === target;
      });
      return file?.path ?? null;
    },
    [note, files],
  );

  const openWikiLink = useCallback(
    (label: string, path: string | null) => {
      if (path) {
        void openFile(path);
        return;
      }
      setMessage(
        t('The link "{label}" does not point to a file yet.', { label }),
      );
    },
    [openFile, t],
  );

  const openLink = useCallback(
    (url: string) => {
      if (url.startsWith("#")) return;
      if (/^(https?:|mailto:|tel:)/i.test(url)) {
        void api.openPath(url).catch((error) => setMessage(String(error)));
        return;
      }
      if (!note) return;
      const target = resolveRelativePath(note.path, url);
      const known = files.find((file) => sameFilePath(file.path, target));
      if (known) {
        void openFile(known.path);
        return;
      }
      void api.openPath(target).catch((error) => setMessage(String(error)));
    },
    [note, files, openFile],
  );

  useEffect(() => {
    setExpandedTasks(new Set());
  }, [selectedProject]);

  useEffect(() => {
    let active = true;
    let timer: number | undefined;
    const unlistenPromise = listen("memory-changed", () => {
      if (!active) return;
      if (timer) window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void refresh().then(async () => {
          if (!active || !selectedPath) return;
          if (noteDirty) {
            setMessage(
              t("The file changed outside Beta Start. Save your changes or reopen the note."),
            );
            return;
          }
          try {
            const detail = await api.readNote(selectedPath);
            if (active) setNote(detail);
          } catch {
            if (active) {
              setNote(null);
              setSelectedPath(null);
              setMessage(t("The open note is no longer available."));
            }
          }
        });
      }, 250);
    });

    return () => {
      active = false;
      if (timer) window.clearTimeout(timer);
      void unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [refresh, selectedPath, noteDirty, t]);

  const noteVersion = useRef(0);
  const autosaveBusy = useRef(false);

  const updateNote = (patch: Partial<NoteDetail>) => {
    noteVersion.current += 1;
    setNote((current) => (current ? { ...current, ...patch } : current));
    setNoteDirty(true);
  };

  const persistNote = useCallback(async (current: NoteDetail) => {
    if (current.kind === "task") {
      await api.updateTask(current.id ?? "", {
        title: current.title,
        status: current.status ?? "todo",
        priority: current.priority ?? "medium",
        due: current.due ?? "",
        remind: current.remind ?? "",
        tags: current.tags,
        body: current.body,
        parent: current.parent ?? "",
      });
    } else {
      await api.saveNote(current.path, current.title, current.tags, current.body);
    }
  }, []);

  // Autosave: evita perder edições quando o app é fechado ou a nota é trocada.
  useEffect(() => {
    if (!note || !noteDirty || editorMode !== "edit") return;
    const snapshot = note;
    const version = noteVersion.current;
    const timer = window.setTimeout(() => {
      if (autosaveBusy.current) return;
      autosaveBusy.current = true;
      void (async () => {
        try {
          await persistNote(snapshot);
          if (noteVersion.current === version) {
            setNoteDirty(false);
            setMessage(t("Saved automatically."));
          }
        } catch (error) {
          setMessage(String(error));
        } finally {
          autosaveBusy.current = false;
        }
      })();
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [note, noteDirty, editorMode, persistNote, t]);

  const saveNote = useCallback(async () => {
    if (!note) return;
    setBusy(true);
    setMessage(null);
    try {
      await persistNote(note);
      await refresh();
      setNote(await api.readNote(note.path));
      noteVersion.current += 1;
      setNoteDirty(false);
      setMessage(t("Saved."));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [note, refresh, persistNote, t]);

  const createNote = useCallback(async () => {
    setBusy(true);
    try {
      const created = await api.createNote(projectFilter, t("New note"));
      setSelectedPath(created.path);
      setNote(created);
      setNoteDirty(false);
      setEditorMode("edit");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [projectFilter, refresh, t]);

  const createDailyNote = useCallback(async () => {
    const now = new Date();
    const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
    const title = t("Journal {date}", { date: today });
    const existing = files.find((file) => file.title === title);
    if (existing) {
      await openFile(existing.path);
      return;
    }
    setBusy(true);
    try {
      const created = await api.createNote(projectFilter, title);
      const withBody = await api.saveNote(
        created.path,
        title,
        ["journal"],
        `# ${title}\n\n`,
      );
      setSelectedPath(created.path);
      setNote(withBody);
      setNoteDirty(false);
      setEditorMode("edit");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [files, openFile, projectFilter, refresh, t]);

  const deleteNote = useCallback(async () => {
    if (!note) return;
    if (
      !window.confirm(
        t('Delete "{title}"?\n\nThe file will be removed from disk.', {
          title: note.title,
        }),
      )
    ) {
      return;
    }
    setBusy(true);
    try {
      await api.deleteMemoryFile(note.path);
      setNote(null);
      setSelectedPath(null);
      setNoteDirty(false);
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [note, refresh, t]);

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

  const linkFolder = useCallback(async () => {
    setBusy(true);
    setMessage(null);
    try {
      await api.setNativeDialogOpen(true);
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("Select the project folder"),
      });
      if (!selected || Array.isArray(selected)) return;
      setMessage(t("Indexing folder…"));
      const project = await api.linkProject(selected);
      await refresh();
      setSelectedProject(project.slug);
      const indexed = (await api.listMemoryFiles(project.slug)).filter(
        (file) => file.kind !== "project",
      );
      setMessage(
        t('Folder linked as project "{name}" — {count} .md file(s) indexed.', {
          name: project.title,
          count: indexed.length,
        }),
      );
    } catch (error) {
      setMessage(String(error));
    } finally {
      await api.setNativeDialogOpen(false).catch(() => {});
      setBusy(false);
    }
  }, [refresh, t]);

  const unlinkSelectedProject = useCallback(async () => {
    const project = projects.find((item) => item.slug === selectedProject);
    if (!project || project.mode !== "linked") return;
    const confirmed = window.confirm(
      t('Unlink "{title}"?\n\nFiles in the folder will not be deleted.', {
        title: project.title,
      }),
    );
    if (!confirmed) return;
    setBusy(true);
    try {
      await api.unlinkProject(project.slug);
      setSelectedProject("all");
      setNote(null);
      setSelectedPath(null);
      setNoteDirty(false);
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [projects, selectedProject, refresh, t]);

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
      if (
        !window.confirm(
          t('Delete the task "{title}"?\n\nThe file will be removed from disk.', {
            title: task.title,
          }),
        )
      ) {
        return;
      }
      setBusy(true);
      try {
        await api.deleteTask(task.id);
        if (selectedPath === task.filePath) {
          setNote(null);
          setSelectedPath(null);
          setNoteDirty(false);
        }
        await refresh();
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [refresh, selectedPath, t],
  );

  const grouped = useMemo(() => {
    const map: Record<string, MemoryFile[]> = {};
    for (const file of files) {
      const normalized = normalizePath(file.path).toLowerCase();
      if (file.kind === "project" && normalized.includes("/.betastart/")) {
        continue;
      }
      (map[file.project] ??= []).push(file);
    }
    return map;
  }, [files]);

  const selectedProjectInfo = useMemo(
    () => projects.find((project) => project.slug === selectedProject) ?? null,
    [projects, selectedProject],
  );
  const selectedLinkedProject =
    selectedProjectInfo?.mode === "linked" ? selectedProjectInfo : null;

  const projectRoots = useMemo(() => {
    const map: Record<string, string> = {};
    for (const project of projects) {
      if (project.rootPath) map[project.slug] = project.rootPath;
    }
    if (workspacePath) map.global = joinPath(workspacePath, "global");
    return map;
  }, [projects, workspacePath]);

  const projectTitle = useCallback(
    (slug: string) => {
      if (slug === "global") return "Global";
      return projects.find((project) => project.slug === slug)?.title ?? slug;
    },
    [projects],
  );

  const fileTree = useMemo(() => {
    const result: Record<
      string,
      { folder: string; files: { file: MemoryFile; rel: string }[] }[]
    > = {};
    for (const [project, list] of Object.entries(grouped)) {
      const folders = new Map<string, { file: MemoryFile; rel: string }[]>();
      for (const file of list) {
        const rel = relativeToRoot(file.path, projectRoots[project]);
        const folder = folderOf(rel);
        const bucket = folders.get(folder) ?? [];
        bucket.push({ file, rel });
        folders.set(folder, bucket);
      }
      result[project] = [...folders.entries()]
        .sort(([left], [right]) => {
          if (left === "") return -1;
          if (right === "") return 1;
          return left.localeCompare(right);
        })
        .map(([folder, entries]) => ({
          folder,
          files: entries.sort((left, right) => left.rel.localeCompare(right.rel)),
        }));
    }
    return result;
  }, [grouped, projectRoots]);

  const roots = useMemo(() => tasks.filter((task) => !task.parentId), [tasks]);
  const childrenByParent = useMemo(() => {
    const map = new Map<string, Task[]>();
    for (const task of tasks) {
      if (!task.parentId) continue;
      const list = map.get(task.parentId) ?? [];
      list.push(task);
      map.set(task.parentId, list);
    }
    return map;
  }, [tasks]);

  const toggleTask = useCallback((id: string) => {
    setExpandedTasks((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const renderTask = (task: Task) => {
    const children = childrenByParent.get(task.id) ?? [];
    const isOpen = expandedTasks.has(task.id);
    const hasChildren = children.length > 0;
    return (
      <div className="task-node" key={task.id}>
        <div
          className={`task-card ${task.status === "done" ? "done" : ""} ${hasChildren ? "has-children" : ""}`}
        >
          <div className="task-card-head">
            <strong onClick={() => void openFile(task.filePath)}>{task.title}</strong>
            <span className={`prio prio-${task.priority}`}>
              {PRIORITY_LABEL[task.priority]
                ? t(PRIORITY_LABEL[task.priority])
                : task.priority}
            </span>
          </div>
          {(task.due || task.tags.length > 0) && (
            <div className="task-meta">
              {task.due && (
                <span>⏰ {formatEpoch(task.due, locale)}</span>
              )}
              {task.tags.map((tag) => (
                <span className="tag" key={tag}>
                  {tag}
                </span>
              ))}
            </div>
          )}
          <div className="task-footer">
            <div className="task-actions">
              {STATUSES.map((status) => (
                <button
                  key={status}
                  className={task.status === status ? "active" : ""}
                  disabled={busy}
                  onClick={() => void setStatus(task, status)}
                >
                  {t(STATUS_LABEL[status])}
                </button>
              ))}
              <button disabled={busy} onClick={() => void removeTask(task)}>
                ×
              </button>
            </div>
            {hasChildren && (
              <button
                type="button"
                className={`task-expand ${isOpen ? "open" : ""}`}
                title={
                  isOpen
                    ? t("Collapse subtasks")
                    : t("Show {count} subtask(s)", { count: children.length })
                }
                aria-expanded={isOpen}
                onClick={() => toggleTask(task.id)}
              >
                <span className="task-expand-chevron">{isOpen ? "▾" : "▸"}</span>
                {children.length}
              </button>
            )}
          </div>
        </div>
        {hasChildren && isOpen && (
          <div className="subtask-list">
            {children.map((child) => renderTask(child))}
          </div>
        )}
      </div>
    );
  };

  return (
    <section className="memory">
      <div className="mem-header">
        <h1>{t("Memory & Tasks")}</h1>
        <div className="inline">
          <button className="secondary" onClick={onBack}>
            {t("Back")}
          </button>
          <button className="secondary" disabled={busy} onClick={() => void refresh()}>
            {t("Refresh")}
          </button>
        </div>
      </div>

      <div className="mem-toolbar">
        <input
          className="text-input"
          placeholder={t("Search memory…")}
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        <button className="primary" disabled={busy} onClick={() => void createNote()}>
          {t("New note")}
        </button>
        <button
          className="secondary"
          disabled={busy}
          onClick={() => void createDailyNote()}
          title={t("Open or create today's note")}
        >
          {t("Today")}
        </button>
      </div>

      <div className="mem-body">
        <aside className="mem-sidebar">
          <div className="mem-projects">
            <h2>{t("Projects")}</h2>
            <button
              className={selectedProject === "all" ? "active" : ""}
              onClick={() => setSelectedProject("all")}
            >
              {t("All")}
            </button>
            <button
              className={selectedProject === "global" ? "active" : ""}
              onClick={() => setSelectedProject("global")}
            >
              {t("Global")}
            </button>
            {projects.map((project) => (
              <button
                key={project.slug}
                className={`${selectedProject === project.slug ? "active" : ""} ${project.available ? "" : "unavailable"}`}
                title={project.rootPath ?? undefined}
                onClick={() => setSelectedProject(project.slug)}
              >
                <span className="mem-project-name">{project.title}</span>
                {project.mode === "linked" && (
                  <span className="mem-project-badge">{t("folder")}</span>
                )}
              </button>
            ))}
            <div className="inline">
              <input
                className="text-input"
                placeholder={t("New project")}
                value={newProject}
                onChange={(event) => setNewProject(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    void createProject();
                  }
                }}
              />
              <button
                className="secondary"
                disabled={busy || !newProject.trim()}
                title={t("Create project (Enter)")}
                onClick={() => void createProject()}
              >
                {t("Create")}
              </button>
            </div>
            <button
              className="secondary mem-link-btn"
              disabled={busy}
              title={t("Index an existing folder without moving the files")}
              onClick={() => void linkFolder()}
            >
              {t("Link folder…")}
            </button>
            {selectedLinkedProject && (
              <div className="mem-project-actions">
                <button
                  className="secondary"
                  disabled={busy}
                  onClick={() => {
                    if (selectedLinkedProject.rootPath) {
                      void api
                        .openPath(selectedLinkedProject.rootPath)
                        .catch((error) => setMessage(String(error)));
                    }
                  }}
                >
                  {t("Open folder")}
                </button>
                <button
                  className="secondary"
                  disabled={busy}
                  onClick={() => void unlinkSelectedProject()}
                >
                  {t("Unlink")}
                </button>
              </div>
            )}
          </div>

          <div className="mem-files">
            <h2>{searchHits ? t("Search") : t("Files")}</h2>
            {searchHits ? (
              searchHits.map((hit) => {
                const rel = relativeToRoot(
                  hit.filePath,
                  projectRoots[hit.project],
                );
                return (
                  <button
                    key={hit.filePath}
                    className="mem-file"
                    title={rel}
                    onClick={() => void openFile(hit.filePath)}
                  >
                    <span className={`kind kind-${hit.kind}`}>{kindIcon(hit.kind)}</span>
                    <span className="mem-file-text">
                      <strong>{hit.title}</strong>
                      <small>
                        {projectTitle(hit.project)} · {rel}
                      </small>
                    </span>
                  </button>
                );
              })
            ) : Object.keys(fileTree).length === 0 ? (
              <div className="mem-empty-small">{t("No .md files found.")}</div>
            ) : (
              Object.entries(fileTree).map(([project, folders]) => (
                <div key={project}>
                  <div className="mem-group">{projectTitle(project)}</div>
                  {folders.map((group) => (
                    <div className="mem-folder-block" key={group.folder || "__root"}>
                      {group.folder && (
                        <div className="mem-folder" title={group.folder}>
                          {folderLabel(group.folder)}
                        </div>
                      )}
                      {group.files.map(({ file, rel }) => (
                        <button
                          key={file.path}
                          className={`mem-file ${selectedPath === file.path ? "active" : ""}`}
                          title={rel}
                          onClick={() => void openFile(file.path)}
                        >
                          <span className={`kind kind-${file.kind}`}>{kindIcon(file.kind)}</span>
                          <span className="mem-file-text">
                            <strong>{file.title}</strong>
                            <small>{t(file.kind)}</small>
                          </span>
                        </button>
                      ))}
                    </div>
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
                {editorMode === "edit" ? (
                  <input
                    className="note-title"
                    value={note.title}
                    onChange={(event) => updateNote({ title: event.target.value })}
                  />
                ) : (
                  <h1 className="note-title-read">
                    <MarkdownInline
                      text={note.title || t("Untitled")}
                      resolveWikiLink={resolveWikiLink}
                      onOpenWikiLink={openWikiLink}
                      onOpenUrl={openLink}
                    />
                  </h1>
                )}
                <div className="mem-editor-actions">
                  <div className="segmented">
                    <button
                      className={editorMode === "read" ? "active" : ""}
                      onClick={() => setEditorMode("read")}
                    >
                      {t("Read")}
                    </button>
                    <button
                      className={editorMode === "edit" ? "active" : ""}
                      onClick={() => setEditorMode("edit")}
                    >
                      {t("Edit")}
                    </button>
                  </div>
                  {editorMode === "edit" && (
                    <button className="primary" disabled={busy} onClick={() => void saveNote()}>
                      {t("Save")}
                    </button>
                  )}
                  <button className="secondary" disabled={busy} onClick={() => void deleteNote()}>
                    {t("Delete")}
                  </button>
                </div>
              </div>

              <div className="note-meta">
                <span className="kind-badge">{t(note.kind)}</span>
                {note.project && <span className="kind-badge">{note.project}</span>}
                {note.id && <span className="kind-badge">#{note.id.slice(0, 8)}</span>}
              </div>

              {note.links.length > 0 && (
                <div className="note-links">
                  {note.links.map((link) => (
                    <button
                      key={link.label}
                      className={`link-chip ${link.path ? "" : "unresolved"}`}
                      title={
                        link.path
                          ? t("Open {label}", { label: link.label })
                          : t('Target "{label}" not found', { label: link.label })
                      }
                      onClick={() => openWikiLink(link.label, link.path)}
                    >
                      [[{link.label}]]
                    </button>
                  ))}
                </div>
              )}

              {backlinks.length > 0 && (
                <div className="backlinks">
                  <span className="field-label">{t("Linked from")}</span>
                  <div className="note-links">
                    {backlinks.map((item) => (
                      <button
                        key={item.path}
                        className="link-chip"
                        title={item.path}
                        onClick={() => void openFile(item.path)}
                      >
                        {item.title || item.name}
                      </button>
                    ))}
                  </div>
                </div>
              )}

              {editorMode === "edit" ? (
                <>
                  {note.kind === "task" && (
                    <div className="task-fields">
                      <select
                        className="text-input"
                        value={note.status ?? "todo"}
                        onChange={(event) => updateNote({ status: event.target.value })}
                      >
                        {STATUSES.map((status) => (
                          <option key={status} value={status}>
                            {t(STATUS_LABEL[status])}
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
                            {t(label)}
                          </option>
                        ))}
                      </select>
                      <input
                        className="text-input"
                        type="datetime-local"
                        title={t("Due date")}
                        value={toLocalInput(note.due)}
                        onChange={(event) => updateNote({ due: event.target.value })}
                      />
                      <input
                        className="text-input"
                        type="datetime-local"
                        title={t("Reminder")}
                        value={toLocalInput(note.remind)}
                        onChange={(event) => updateNote({ remind: event.target.value })}
                      />
                    </div>
                  )}

                  <input
                    className="text-input"
                    placeholder={t("tags, comma separated")}
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
                    placeholder={t("Write here… use # headings, **bold**, lists, [[Note name]], and links")}
                  />
                </>
              ) : (
                <>
                  {note.kind === "task" && (
                    <div className="task-facts">
                      <span className={`kind-badge status-${note.status ?? "todo"}`}>
                        {STATUS_LABEL[note.status ?? "todo"]
                          ? t(STATUS_LABEL[note.status ?? "todo"])
                          : note.status}
                      </span>
                      <span className={`kind-badge prio-${note.priority ?? "medium"}`}>
                        {PRIORITY_LABEL[note.priority ?? "medium"]
                          ? t(PRIORITY_LABEL[note.priority ?? "medium"])
                          : note.priority}
                      </span>
                      {note.due && (
                        <span className="kind-badge">
                          {t("Due {date}", {
                            date: formatIsoDisplay(note.due, locale),
                          })}
                        </span>
                      )}
                      {note.remind && (
                        <span className="kind-badge">
                          {t("Reminder {date}", {
                            date: formatIsoDisplay(note.remind, locale),
                          })}
                        </span>
                      )}
                    </div>
                  )}

                  {note.tags.length > 0 && (
                    <div className="note-tags">
                      {note.tags.map((tag) => (
                        <span className="tag" key={tag}>
                          #{tag}
                        </span>
                      ))}
                    </div>
                  )}

                  <MarkdownView
                    source={note.body}
                    resolveWikiLink={resolveWikiLink}
                    onOpenWikiLink={openWikiLink}
                    onOpenUrl={openLink}
                  />
                </>
              )}
            </>
          ) : (
            <div className="mem-empty">{t("Select a file or create a note.")}</div>
          )}
          {message && <p className="settings-msg">{message}</p>}
        </main>

        <aside className="mem-tasks">
          <div className="mem-tasks-head">
            <div className="segmented">
              <button
                className={rightPanel === "tasks" ? "active" : ""}
                onClick={() => setRightPanel("tasks")}
              >
                Tasks
              </button>
              <button
                className={rightPanel === "graph" ? "active" : ""}
                onClick={() => setRightPanel("graph")}
              >
                {t("Graph")}
              </button>
            </div>
            {rightPanel === "tasks" && (
              <div className="segmented">
                <button
                  className={view === "list" ? "active" : ""}
                  onClick={() => setView("list")}
                >
                  {t("List")}
                </button>
                <button
                  className={view === "kanban" ? "active" : ""}
                  onClick={() => setView("kanban")}
                >
                  Kanban
                </button>
              </div>
            )}
          </div>

          {rightPanel === "graph" ? (
            <Graph
              project={projectFilter}
              version={graphVersion}
              onOpen={(path) => void openFile(path)}
            />
          ) : (
            <>
          <div className="task-form">
            <input
              className="text-input"
              placeholder={t("New task… (Enter to create)")}
              value={taskTitle}
              onChange={(event) => setTaskTitle(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void createTask();
                }
              }}
            />
            <div className="inline">
              <select
                className="text-input"
                value={taskPriority}
                onChange={(event) => setTaskPriority(event.target.value)}
              >
                <option value="high">{t("High")}</option>
                <option value="medium">{t("Medium")}</option>
                <option value="low">{t("Low")}</option>
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
                placeholder={t("tags, comma separated")}
                value={taskTags}
                onChange={(event) => setTaskTags(event.target.value)}
              />
              <select
                className="text-input"
                value={taskParent}
                onChange={(event) => setTaskParent(event.target.value)}
              >
                <option value="">{t("No parent")}</option>
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
              {t("Add task")}
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
                    <div className="task-group-title">{t(STATUS_LABEL[status])}</div>
                    {list.map((task) => renderTask(task))}
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="kanban">
              {STATUSES.map((status) => (
                <div className="kanban-col" key={status}>
                  <div className="kanban-title">{t(STATUS_LABEL[status])}</div>
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
                              → {t(STATUS_LABEL[item])}
                            </button>
                          ))}
                        </div>
                      </div>
                    ))}
                </div>
              ))}
            </div>
          )}
            </>
          )}
        </aside>
      </div>
    </section>
  );
}
