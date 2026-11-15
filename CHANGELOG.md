# Changelog

## 0.3.0

Redesign visual, temas de accent, regras de janela visuais e melhorias de UX no MCP.

### Adicionado

- **Novo sistema visual**: grafite neutro com accent teal, tipografia nativa do
  Windows, ícones SVG próprios, favicon e marca atualizados.
- **Temas de accent**: teal, azul, violeta, rosa, âmbar e ardósia, cada um com
  variantes para os temas claro e escuro. A escolha é salva imediatamente.
- **Tema claro** com contraste e superfícies reajustados.
- **Editor visual de regras por app**: prévia do monitor, arraste para desenhar
  a área, presets (metades, centro, cantos), captura de janela aberta e
  coordenadas avançadas opcionais.
- **Lista visual de regras**: mini-mapa da região, estado ativo/desativado,
  edição, switch de ativação e exclusão.
- **MCP**: “Últimas chamadas” agora é um painel retrátil e compacto, fechado por
  padrão.
- **MCP**: chaves revogadas ficam em um painel retrátil e podem ser excluídas
  permanentemente (`mcp_delete_client`). Apenas chaves revogadas podem ser
  apagadas.
- **Configurações**: índice de seções com scroll spy, barra fixa de salvamento,
  indicador de alterações não salvas e seletor de accent.
- **Busca**: estado vazio com exemplos, botão de limpar, tipos de resultado,
  ação contextual (“Open”, “Copy”, “Run”…), ARIA de combobox/listbox e
  scroll automático do item selecionado.

### Alterado

- Janela do Window Manager ampliada para 900×640.
- Componente compartilhado `Select` com chevron em SVG em vez de
  `background-image`.
- Notas usam a fonte de interface no corpo; o H1 duplicado do Markdown é ocultado.
- Títulos de task viraram botões acessíveis; status têm `aria-pressed`.
- Botões destrutivos usam `danger`; labels, `aria-label`s, foco visível,
  `prefers-reduced-motion` e contraste foram revisados.
- Mensagens de ações em MCP e Window Manager agora aparecem como toast.
- Tema, accent e idioma são salvos imediatamente; o restante usa a barra de
  salvamento com estado explícito.

### Corrigido

- Dropdowns não repetem mais a setinha dentro do popup nativo do WebView2.
- Painéis retráteis do MCP não são mais esmagados pelo flex layout.
- Documentação atualizada para os novos ícones e fluxos.

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
