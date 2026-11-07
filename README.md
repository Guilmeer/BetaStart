# Beta Start

Launcher/centro de produtividade estilo Raycast — gratuito e open-source (MIT).
Uma janela flutuante central para abrir apps, buscar arquivos, rodar comandos de extensões,
gerenciar janelas e acessar um "segundo cérebro" de memória + tasks, integrável a IAs via MCP.

- Arquitetura completa: [`ARCH.md`](./ARCH.md)
- Prioridades e ordem de produção: [`ROADMAP.md`](./ROADMAP.md)

## Stack

- **Tauri 2** (backend em Rust + frontend React/TypeScript)
- **Deno (`deno_core`)** embutido para as extensões (próximas fases)
- **SQLite** para índices/clipboard/tasks/tokens + **JSON** para configuração
- **MIT**

## Pré-requisitos (Windows)

- Node.js 20+ e npm
- Rust (stable) — instale em <https://rustup.rs>
- Visual Studio Build Tools com "Desktop development with C++"

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

## Estrutura do repositório

```
src-tauri/src/
├── main.rs            # entrypoint
├── lib.rs             # builder Tauri, plugins, setup
├── commands.rs        # comandos (IPC) expostos ao frontend
├── error.rs           # AppError unificado
├── state.rs           # AppState (config + conexão SQLite)
├── core/
│   ├── config.rs      # config.json (load/save/patch)
│   ├── paths.rs       # diretórios do app e do workspace
│   ├── window.rs      # show/hide/toggle da janela
│   ├── tray.rs        # ícone e menu da bandeja
│   └── hotkey.rs      # hotkey global do launcher
├── db/
│   ├── mod.rs         # abertura + migrações
│   ├── schema.rs      # schema SQL completo (v1)
│   └── apps.rs        # índice/busca de apps
└── platform/
    ├── mod.rs         # tipos + launch (Windows/Linux)
    ├── apps.rs        # varredura do Menu Iniciar (.lnk)
    └── windows.rs     # enumeração de janelas (Win32)
src/                   # frontend React
```

## Estado atual — Fase 0 (Fundação)

- [x] Config persistente (`config.json`)
- [x] Banco SQLite com schema completo e migrações
- [x] Índice de aplicativos do Menu Iniciar + busca
- [x] Janela flutuante (arrastável, sem bordas, always-on-top)
- [x] Bandeja do sistema com menu (Abrir / Reindexar / Sair)
- [x] Fechar a janela → vai para a bandeja
- [x] Hotkey global do launcher (Alt+Espaço por padrão)
- [x] Enumeração de janelas abertas (Win32)
- [x] Onboarding inicial (substituir Win vs Win+X)
- [ ] Interceptação real da tecla Win (low-level hook) — próxima fase
- [ ] Busca de arquivos e clipboard — próxima fase
- [ ] Extensões (Deno), memória/tasks, grafo e MCP — fases seguintes

## Próximos passos

Siga o [`ROADMAP.md`](./ROADMAP.md): Fase 1 (launcher completo), Fase 2 (extensões),
Fase 3 (janelas), Fase 4 (memória + tasks), Fase 5 (grafo), Fase 6 (MCP), Fase 7 (distribuição).
