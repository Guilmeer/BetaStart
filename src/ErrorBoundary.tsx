import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Evita que um erro de renderização deixe a janela em branco (e o app
 * aparentemente "fechado"). Mostra o erro e permite recarregar a interface.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: unknown): State {
    return {
      error: error instanceof Error ? error : new Error(String(error)),
    };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[betastart] erro na interface:", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;

    return (
      <div className="launcher">
        <div className="crash">
          <h1>Algo deu errado nesta tela</h1>
          <p>
            O Beta Start continua aberto. Recarregue a interface para voltar ao
            launcher.
          </p>
          <code className="crash-detail">{this.state.error.message}</code>
          <div className="settings-actions">
            <button
              className="primary"
              onClick={() => window.location.reload()}
            >
              Recarregar
            </button>
            <button
              className="secondary"
              onClick={() => this.setState({ error: null })}
            >
              Tentar novamente
            </button>
          </div>
          <p className="field-hint">
            Se o problema continuar, abra uma issue com a mensagem acima.
          </p>
        </div>
      </div>
    );
  }
}
