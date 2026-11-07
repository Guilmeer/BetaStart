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

## Gerenciador de janelas

Abra **🪟** na barra superior.

### Layouts com hotkey

1. Organize as janelas manualmente (ex.: navegador 25% à direita, Discord 75% à esquerda).
2. Em **Layouts**, dê um nome e um atalho (ex.: `Win+Shift+F7`) e clique em **Salvar atual**.
3. Pressione o atalho a qualquer momento para reaplicar o layout.

O layout guarda, por janela, o processo e a posição relativa à **área útil do monitor**
(excluindo a barra de tarefas), então funciona em diferentes resoluções e monitores.

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
│   ├── schema.rs      # schema SQL (v2)
│   ├── apps.rs        # índice/busca de apps
│   ├── files.rs       # índice/busca de arquivos
│   ├── clipboard.rs   # histórico de clipboard
│   └── extensions.rs  # registro/comandos/permissões de extensões
├── indexer/           # indexação de arquivos em background
├── clipboard_watcher.rs # captura de clipboard em background
├── extensions/
│   ├── mod.rs         # manifesto, instalação (pasta/Git), validação
│   └── runtime.rs     # Deno sidecar, runner, permissões, execução
└── platform/
    ├── mod.rs         # tipos + launch (Windows/Linux)
    ├── apps.rs        # varredura do Menu Iniciar (.lnk)
    └── windows.rs     # enumeração de janelas (Win32)
src/                   # frontend React (launcher, settings, extensions)
examples/              # extensão de exemplo + índice de loja
```

## Estado atual — Fases 0, 1 e 2

**Fase 0 — Fundação**
- [x] Config persistente + migrações automáticas
- [x] Banco SQLite com schema completo (v2)
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
- [x] Tela 🪟 Gerenciador de janelas
- [x] Testes (hotkeys `Win+...`, DAO de layouts/regras, enumeração Win32)

## Próximos passos

Siga o [`ROADMAP.md`](./ROADMAP.md): Fase 4 (memória + tasks), Fase 5 (grafo),
Fase 6 (MCP), Fase 7 (distribuição).
