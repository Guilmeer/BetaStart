# Beta Start — Prioridades e Ordem de Produção

> Documento autocontido para guiar o desenvolvimento e ser usado como contexto em outras
> conversas/IAs. Arquitetura detalhada em `ARCH.md`.

## Resumo do projeto

- **Nome:** Beta Start (identificador: `betastart`)
- **O que é:** launcher/centro de produtividade estilo Raycast, gratuito e open-source (MIT),
  residente na bandeja. Substitui o Menu Iniciar de forma eficiente e concentra apps, arquivos,
  extensões, gerenciamento de janelas, memória/tasks e integração com IAs via MCP.
- **Plataforma:** Windows primeiro → Linux → macOS.
- **Stack:** Tauri 2 (backend Rust + frontend React/TypeScript).
- **Extensões:** TypeScript/JavaScript rodando em Deno (`deno_core`) embutido, processo isolado.
- **Persistência:** SQLite (índices/clipboard/tasks/tokens) + JSON (config). Memória = arquivos Markdown reais em disco.
- **MCP:** stdio + HTTP/SSE local, token + escopos aprovados.

---

## Prioridades (por valor entregue)

### P0 — Núcleo essencial (MVP, sem ele não é o produto)

1. **Janela flutuante central** — arrastável, abre por hotkey ou tecla Win, fecha ao clicar fora → system tray.
2. **Launcher / substituição do Menu Iniciar** — busca de **apps**, **arquivos**, **comandos de extensões**, **calculadora inline** e **histórico de clipboard**.
3. **Escolha no primeiro uso** — o usuário decide se a tecla Win abre o app (substituindo o Start) ou se usa Win+X; fallback Win+Shift sempre abre o Start real.
4. **Sistema de extensões** — runtime isolado (Deno), manifesto com permissões, instalação via **loja integrada + Git/URL + pasta local**.
5. **Gerenciador de janelas** — hotkeys para posicionar janelas (tamanhos/posições), regras por aplicativo, **multi-monitor**, **salvar/restaurar layout**.

### P1 — Diferenciais (transforma em "segundo cérebro")

6. **Ambiente de memória** — arquivos Markdown reais em workspace (`~/Documents/BetaStart`), separados por projetos ou memória principal; busca híbrida (palavra-chave + tags), **escopo por projeto** para economizar tokens.
7. **Tasks** — `.md` com frontmatter na mesma árvore; status, prioridade, due date, tags, subtasks, lembretes, visualização em **lista e kanban**.
8. **Visualizador de grafo (force-directed)** — nós = projetos/notas/tasks/tags/arquivos; arestas = links wiki, pai→filho, tags, projeto.
9. **Porta MCP** — servidor MCP (stdio + HTTP/SSE) com tools para memória, tasks, janelas, extensões, busca e settings; token + escopos por cliente.

### P2 — Futuro (adiado explicitamente)

- Busca semântica por embeddings/RAG.
- Virtual desktops no gerenciador de janelas.
- Acesso MCP remoto (hoje só localhost).
- Sincronização multi-dispositivo.
- Linux e macOS.

---

## Ordem de produção (fases e módulos)

> A ordem é por **dependência**: só construir depois que o módulo de baixo existir.

| Fase | Módulos | Entregável / "Definition of Done" |
|---|---|---|
| **0 — Fundação** | 1 `core` (ciclo de vida, tray, single-instance, config, log) · 2 `platform` (hotkey global, hook de teclado, enumeração/movimentação de janelas, apps instalados) · 3 `db` (SQLite, FTS5) | App abre, fica na bandeja, lê/grava `config.json`, hook de teclado captura a tecla Win, lista janelas e apps instalados. |
| **1 — Launcher MVP** | 4 `ui-shell` (shell React) · 5 `indexer` (índice apps+arquivos) · 6 `launcher` (command palette, busca) · 7 `clipboard` | Janela flutuante abre/fecha corretamente, busca apps/arquivos/comandos/calculadora/clipboard, escolha Win vs Win+X funcionando. |
| **2 — Extensões** | 8 `extensions` (runtime Deno, manifesto, permissões, loja/instalação, ponte de comandos) | Extensão de exemplo instalável via loja/Git/pasta, roda isolada, permissões aprovadas na instalação, comandos aparecem no launcher. |
| **3 — Janelas** | 9 `window-manager` (layouts por hotkey, regras por app, salvar/restaurar) | Hotkey reposiciona janelas (ex.: Win+Shift+F7 → navegador 25% direita, Discord 75% esquerda), regras por app, multi-monitor, salvar/restaurar layout. |
| **4 — Memória + Tasks** | 10 `memory` (workspace, projetos, frontmatter, busca híbrida) · 11 `tasks` (índice, lista/kanban, lembretes) | Workspace navegável na UI, CRUD de notas/tasks via arquivos, busca por projeto + tags, kanban/lista, lembretes. |
| **5 — Grafo** | 12 `graph` (modelo + render force-directed) | Painel lateral com grafo force-directed de notas/tasks/tags/projetos + links. |
| **6 — MCP** | 13 `mcp` (servidor stdio+SSE, auth, tools) | Cliente externo (Claude Desktop/Cursor) conecta via token, executa tools de memória/tasks/janelas/extensões/busca/settings respeitando escopos. |
| **7 — Distribuição** | 14 `updater` · 15 `packaging` | Instalador Windows + auto-update via GitHub Releases. |

**Regra de ouro por fase:** não avança para a próxima fase sem a fase anterior "done" e testável.

---

## Ordem de implementação recomendada (detalhe fino do MVP)

1. `core` (config, tray, single-instance) — sem UI ainda.
2. `platform/windows` (hotkey global + hook Win + enumeração de apps/janelas).
3. `db` (migrações + primeiras tabelas: `apps_index`, `clipboard_history`).
4. `ui-shell` (janela flutuante, fechar→tray, arrastar).
5. `indexer` (apps + arquivos) → `launcher` (busca) → `clipboard`.
6. `extensions` (runtime + manifesto + um comando de teste).
7. `window-manager`.
8. Em seguida: `memory` → `tasks` → `graph` → `mcp` → `updater`/`packaging`.

---

## Definições rápidas (para dar contexto a outras IAs)

- **Workspace de memória:** pasta real com `global/` (memória principal) e `projects/<slug>/`.
  Toda nota/task é um `.md`; o tipo vem do frontmatter (`note`, `task`, `project`).
- **Task:** `.md` com frontmatter `status | priority | due | parent | tags | project`.
- **Grafo:** nós (`project`, `note`, `task`, `tag`, `file`) + arestas (`parent`, `link`, `tag`, `project`).
- **MCP:** token com hash em `mcp_clients`, escopos por cliente, servidor restrito a `127.0.0.1`.
- **Extensões:** `extension.json` declara comandos, `searchProviders` e permissões
  (`fs:read/write`, `network`, `shell`, `clipboard`, `memory`, `tasks`, `windows`, `search`).
