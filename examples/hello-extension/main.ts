import { registerCommand } from "@betastart/sdk";

registerCommand({
  id: "hello",
  title: "Hello Beta Start",
  run: () => ({ type: "message", text: "Hello from Beta Start! 👋" }),
});

registerCommand({
  id: "copy-date",
  title: "Copiar data atual",
  run: () => ({ type: "copy", text: new Date().toISOString() }),
});
