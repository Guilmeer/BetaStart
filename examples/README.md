# Extensões de exemplo

`hello-extension/` é uma extensão mínima que registra dois comandos:

- **Hello Beta Start** → mostra uma mensagem (toast).
- **Copiar data atual** → copia a data ISO para a área de transferência
  (requer a permissão `clipboard:write`).

## Como instalar

No app, abra **🧩 Extensões** e use **Pasta local** apontando para:

```
E:\DOCUMENTOS\Opencode\BetterStart\examples\hello-extension
```

Aprove a permissão `clipboard:write`. Depois, digite `Hello` ou `data` no launcher.

## Loja de exemplo

`store/index.json` é um índice de loja local. Em **Extensões → Loja**, cole o
caminho absoluto desse arquivo (ou a URL de um `index.json` remoto) e clique em
**Carregar**.

## Estrutura de uma extensão

```
minha-extensao/
├── extension.json   # manifesto (id, entry, comandos, permissões)
└── main.ts          # código (TS/JS) que importa @betastart/sdk
```

O runtime executa cada comando em um **processo Deno isolado**, com apenas as
permissões aprovadas pelo usuário. O comando retorna uma ação:

- `{ type: "message", text }` → mostra um toast
- `{ type: "copy", text }` → copia (requer `clipboard:write`)
- `{ type: "open", path }` → abre um caminho (requer `shell`)
