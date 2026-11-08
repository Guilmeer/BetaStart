# Beta Start

Launcher/centro de produtividade estilo Raycast — gratuito e open-source (MIT).
Uma janela flutuante central para abrir apps, buscar arquivos, rodar comandos de extensões,
gerenciar janelas e acessar um "segundo cérebro" de memória + tasks, integrável a IAs via MCP.

- Arquitetura completa: [`ARCH.md`](./ARCH.md)
- Prioridades e ordem de produção: [`ROADMAP.md`](./ROADMAP.md)

## Stack

- **Tauri 2** (backend em Rust + frontend React/TypeScript)
- **Deno como processo sidecar** (isolado) para as extensões TS/JS
- **SQLite** para índices/clipboard/tasks/tokens + **JSON** para configuração
- **MIT**

## Pré-requisitos (Windows)

- Node.js 20+ e npm
- Rust (stable) — instale em <https://rustup.rs>
- Visual Studio Build Tools com "Desktop development with C++"
- Para extensões: Deno (o app baixa automaticamente em **Extensões**, se necessário)

## Desenvolvimento

```bash
npm install
npm run tauri dev
```

A janela flutuante abre centralizada. Atalho padrão para abrir/fechar: **Alt + Espaço**.
Fechar a janela (X/click fora) só a esconde — o app continua na bandeja do sistema.
Para encerrar de vez, use **Sair** no menu da bandeja.

## Build

```bash
npm run tauri build
```

## Como usar

- **Alt + Espaço** abre/fecha o launcher.
- Digite para buscar **apps**, **arquivos**, **clipboard**, comandos de extensões e
  **calculadora** (ex.: `12*8+3`).
- **🧠 Memória & Tasks**: notas, projetos, busca, tasks com subtasks, lista/kanban e lembretes.
- **🪟 Gerenciador de janelas**: salvar/aplicar layouts, regras por app e multi-monitor.
- **⚙ Configurações**: trocar o atalho e o modo da tecla Win a qualquer momento.
- **🧩 Extensões**: instalar/gerenciar extensões e o runtime Deno.
- **Esc** fecha; fechar a janela manda para a bandeja.

## Extensões

Uma extensão é uma pasta com `extension.json` (manifesto) + código TS/JS:

```
minha-extensao/
├── extension.json   # id, entry, comandos, permissões
└── main.ts          # importa @betastart/sdk
```

Exemplo pronto em [`examples/hello-extension`](./examples/hello-extension).

### Instalar o exemplo

1. Abra **🧩 Extensões**.
2. Em **Pasta local**, use `E:\DOCUMENTOS\Opencode\BetterStart\examples\hello-extension`.
3. Clique em **Instalar**, depois em **Aprovar permissões**.
4. Digite `Hello` ou `data` no launcher.

Também é possível instalar via **URL Git** ou pela **Loja** (um `index.json`, remoto ou
local — veja [`examples/store/index.json`](./examples/store/index.json)).

## Memória & Tasks

Abra **🧠** na barra superior (a janela aumenta automaticamente).

- A **memória principal** fica em `global/`; cada projeto em `projects/<slug>/`.
- Tudo é **Markdown real** com frontmatter YAML — você pode abrir/editar fora do app e
  versionar com Git.
- No app você pode:
  - criar **projetos**, **notas** e **tasks**;
  - editar título, tags e corpo;
  - buscar por **palavra-chave** dentro do projeto (ou em tudo);
  - alternar tasks entre **Lista** e **Kanban**;
  - criar **subtasks** (tarefa pai);
  - definir **vencimento** e **lembrete** (notificação do Windows).
- O índice é atualizado ao abrir a tela, ao salvar e no boot. O botão **Atualizar**
  reindexa manualmente.

Formato de uma task:

```md
---
type: task
id: "uuid"
title: "Minha tarefa"
status: todo            # todo | in_progress | done | blocked | cancelled
priority: high          # high | medium | low
due: "2026-12-01T10:00"
remind: "2026-12-01T09:30"
tags: [trabalho, urgente]
parent: "id-da-task-pai"  # opcional (subtask)
---
Corpo/descrição…
```

### Grafo

No painel direito da tela de memória, alterne entre **Tasks** e **Grafo**. O grafo
force-directed (`d3-force`) mostra projetos, notas, tasks e tags, com arestas de:

- **projeto** — projeto → item;
- **tag** — tag → item;
- **parent** — task pai → subtask;
- **link** — `[[Outra nota]]` encontrado no corpo.

No editor, os links aparecem como **chips**: ciano quando o destino existe e **vermelho**
quando não existe. Destinos não encontrados também aparecem como **nós vermelhos** no grafo.
O grafo é atualizado após salvar/criar/excluir.

Clique em um nó que tenha arquivo para abri-lo. O filtro de projeto também filtra o grafo.

## Gerenciador de janelas

Abra **🪟** na barra superior.

### Layouts com hotkey

1. Organize as janelas manualmente (ex.: navegador 25% à direita, Discord 75% à esquerda).
2. Em **Layouts**, dê um nome. Para o atalho, **clique no campo e pressione a combinação**
   (ex.: `Win+Shift+F7`) — o app entra em modo de escuta e grava o que você usou.
3. Clique em **Salvar atual**.
4. Pressione o atalho a qualquer momento para reaplicar o layout.

> Se a combinação já estiver em uso (pelo launcher, por outro layout ou pelo sistema),
> o campo fica com **borda vermelha** e mostra o motivo. Basta gravar outra.

O layout guarda o processo e a posição em **pixels exatos** (além das frações), então
reaplica com precisão na mesma configuração de monitores e escala proporcionalmente se a
resolução mudar. Layouts salvos antes desta versão devem ser **re-salvos uma vez**.

### Regras por app

Configure app (processo/título/classe), monitor e região (`x/y/w/h` como frações de 0 a 1).
As regras são aplicadas **automaticamente** quando uma janela nova do app aparece
(watcher ~1,2s) ou manualmente em **Aplicar regras agora**.

Exemplo: `chrome` no monitor 1, `x=0.75, y=0, w=0.25, h=1` (direita, 25%).
`discord` no monitor 1, `x=0, y=0, w=0.75, h=1` (esquerda, 75%).

### Multi-monitor

Monitores são enumerados com resolução e área útil; layouts e regras guardam o índice
do monitor. Janelas maximizadas são restauradas antes de serem reposicionadas.

### Runtime e segurança

- Cada comando roda em um **processo Deno separado** (isolamento real).
- Permissões declaradas no manifesto e aprovadas pelo usuário:
  `fs:read`, `fs:write`, `network`, `shell`, `clipboard:read`, `clipboard:write`,
  `memory`, `tasks`, `windows`, `search`.
- O Deno recebe apenas as flags correspondentes às permissões aprovadas.
- Comandos retornam ações seguras: `{ type: "message" }`, `{ type: "copy" }` ou
  `{ type: "open" }` (com checagem de permissão no host).

## Estrutura do repositório

```
src-tauri/src/
├── main.rs            # entrypoint
├── lib.rs             # builder Tauri, plugins, setup
├── commands.rs        # comandos (IPC) expostos ao frontend
├── error.rs           # AppError unificado
├── state.rs           # AppState (config + conexão SQLite)
├── models.rs          # DTOs compartilhados
├── core/
│   ├── config.rs      # config.json (load/save/patch/normalize)
│   ├── paths.rs       # diretórios do app, workspace e runtime
│   ├── window.rs      # show/hide/toggle da janela
│   ├── tray.rs        # ícone e menu da bandeja
│   ├── hotkey.rs      # hotkey global do launcher
│   └── win_hook.rs    # hook da tecla Win (low-level)
├── db/
│   ├── mod.rs         # abertura + migrações
│   ├── schema.rs      # schema SQL (v4)
│   ├── apps.rs        # índice/busca de apps
│   ├── files.rs       # índice/busca de arquivos
│   ├── clipboard.rs   # histórico de clipboard
│   ├── extensions.rs  # registro/comandos/permissões de extensões
│   ├── windows.rs     # layouts/regras de janelas
│   ├── memory.rs      # índice/busca de memória
│   └── tasks.rs       # índice de tasks + lembretes
├── indexer/           # indexação de arquivos em background
├── clipboard_watcher.rs # captura de clipboard em background
├── memory/
│   └── mod.rs         # frontmatter YAML, links wiki, scan/indexação
├── graph/
│   └── mod.rs         # grafo de projetos/notas/tasks/tags/links
├── memory_api.rs      # comandos de memória + tasks + grafo
├── reminder_watcher.rs # notificações de lembretes
├── window_manager/    # layouts, regras, multi-monitor
├── window_watcher.rs  # aplica regras a janelas novas
├── extensions/
│   ├── mod.rs         # manifesto, instalação (pasta/Git), validação
│   └── runtime.rs     # Deno sidecar, runner, permissões, execução
└── platform/
    ├── mod.rs         # tipos + launch (Windows/Linux)
    ├── apps.rs        # varredura do Menu Iniciar (.lnk)
    └── windows.rs     # enumeração de janelas + monitores (Win32)
src/                   # frontend React (launcher, settings, extensões, janelas, memória)
examples/              # extensão de exemplo + índice de loja
```

## Estado atual — Fases 0 a 5

**Fase 0 — Fundação**
- [x] Config persistente + migrações automáticas
- [x] Banco SQLite com schema completo (v4)
- [x] Índice de apps do Menu Iniciar + busca
- [x] Janela flutuante, bandeja, fechar→bandeja
- [x] Hotkey global (Alt+Espaço)
- [x] Enumeração de janelas (Win32)

**Fase 1 — Launcher MVP**
- [x] Hook da tecla Win (modo "substituir")
- [x] Busca de arquivos com indexação em background
- [x] Histórico de clipboard + busca
- [x] Calculadora inline
- [x] Resultados unificados
- [x] Configurações (atalho + modo Win)

**Fase 2 — Extensões**
- [x] Manifesto `extension.json` com validação
- [x] Instalação via pasta local, Git e loja (`index.json`)
- [x] Runtime Deno sidecar com detecção/download automático
- [x] Permissões declaradas e aprovação pelo usuário
- [x] Comandos de extensão no launcher
- [x] SDK `@betastart/sdk` (`registerCommand`, `ui`, `clipboard`)
- [x] Tela 🧩 Extensões (instalar, ativar, aprovar, desinstalar)
- [x] Extensão de exemplo + testes end-to-end

**Fase 3 — Gerenciador de janelas**
- [x] Enumeração de monitores com área útil (multi-monitor)
- [x] Enumeração de janelas com nome do processo
- [x] Salvar/restaurar layout atual
- [x] Hotkey por layout (dispatcher de atalhos)
- [x] Regras por app (processo/título/classe, monitor, frações)
- [x] Watcher que aplica regras a janelas novas
- [x] **Captura de hotkey por gravação** (clique → pressione → salva)
- [x] **Detecção de conflito** com borda vermelha (launcher, layouts, sistema)
- [x] Layout com **pixels exatos** (reaplica sem deslocamento)
- [x] Tela 🪟 Gerenciador de janelas
- [x] Testes (hotkeys `Win+...`, DAO de layouts/regras, enumeração Win32, precisão de pixels)

**Fase 4 — Memória + Tasks**
- [x] Workspace de arquivos Markdown com frontmatter YAML
- [x] Projetos (`projects/<slug>/`) + memória principal (`global/`)
- [x] Indexação em SQLite (`memory_index` + `tasks_index`) e busca por escopo
- [x] CRUD de notas e tasks direto nos arquivos
- [x] Tasks com status, prioridade, due, tags, **subtasks** e lembretes
- [x] Visualização **lista** e **kanban**
- [x] Notificações de lembrete (plugin de notificação)
- [x] Tela 🧠 Memória & Tasks (redimensiona a janela)
- [x] Testes de frontmatter + indexação end-to-end

**Fase 5 — Grafo**
- [x] Índice de links wiki (`[[...]]`) por nota
- [x] Grafo de projetos, notas, tasks e tags (migration v4)
- [x] Arestas de projeto, tag, parent e link
- [x] Visualização force-directed (`d3-force`) no painel lateral
- [x] Clique em um nó abre o arquivo correspondente
- [x] Teste do builder do grafo

## Próximos passos

Siga o [`ROADMAP.md`](./ROADMAP.md): Fase 6 (MCP), Fase 7 (distribuição).
