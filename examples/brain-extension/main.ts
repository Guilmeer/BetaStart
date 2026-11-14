import { projects, registerCommand, tasks, ui } from "@betastart/sdk";

registerCommand({
  id: "list-projects",
  async run() {
    const list = await projects.list();
    ui.toast("Projetos carregados.");
    return {
      type: "message",
      text:
        list.length > 0
          ? list
              .map((project: { title: string; slug: string }) => `${project.title} (${project.slug})`)
              .join(" · ")
          : "Nenhum projeto encontrado.",
    };
  },
});

registerCommand({
  id: "create-task",
  async run() {
    const list = await projects.list();
    const project = list[0]?.slug ?? "global";
    const task = await tasks.create({
      title: "Task criada pela extensão de exemplo",
      project,
      priority: "low",
      tags: ["exemplo"],
    });
    ui.toast(`Task criada em ${project}.`);
    return {
      type: "message",
      text: `Task criada: ${task.title} (${task.id})`,
    };
  },
});
