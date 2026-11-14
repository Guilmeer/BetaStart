# Changelog

## 0.2.0

MCP com custo de token controlado, índice incremental e correções de configuração.

### Adicionado

- **MCP — grafo consultável**: `graph.query`, `graph.neighbors`, `graph.path` e `graph.explain`
  devolvem um subgrafo escopado por termos, com orçamento de tokens (`budget`) e flag `truncated`.
  `graph.get` continua existindo para o grafo inteiro, agora documentado como caro.
- **`memory.context`**: um pacote ranqueado por pergunta (notas + tasks abertas + vizinhança).
- **`memory.read` com modos**: `outline` (títulos + linhas), `range` (intervalo de linhas) e
  `full` com teto de bytes.
- **`initialize.instructions`**: protocolo de leitura entregue ao cliente MCP no handshake.
- **Install guidance** na tela MCP: escreve/atualiza um bloco no `AGENTS.md` do projeto.
- **Reindex incremental**: só arquivos com `mtime` diferente são relidos; triggers do FTS5 mantêm a
  busca em sincronia.
- **Cache do grafo** invalidado por `meta.memory_version`.

### Alterado

- Respostas MCP em **JSON compacto**; caminhos **relativos ao projeto**; ids de nó curtos e estáveis.
- `memory.list` paginado (`limit`/`offset`/`compact`) e `project.brief` com notas rankeadas por
  grau (links + tags) e recência.
- `graph.seeds` agora devolve `{id, label}` para desambiguar notas com o mesmo título.
- Tema e idioma salvam **imediatamente** ao trocar, sem depender do botão Save.
- Um atalho inválido ou uma falha ao registrar o auto-início não impede mais o resto da
  configuração de ser salva.

### Corrigido

- `resources/list` e `resources/read` exigem o escopo `memory` (antes vazavam o workspace inteiro).
- Token MCP inválido no `POST /mcp` devolve **401** em vez de 202 vazio.

### Desempenho (workspace de referência: 123 notas, 84 tasks)

| Chamada | Antes | Depois |
|---|---|---|
| `graph.get` (todos) | ~41.200 tokens | ~15.800 |
| `graph.get` (um projeto) | ~24.400 | ~8.800 |
| `memory.list` (todos) | ~10.300 | ~4.100 |
| pergunta via `graph.query` | — | 0,5k–2k (orçamento) |
| `project.brief` | ~1.200 | ~610 |
