import { Component, type ErrorInfo, type ReactNode } from "react";
import { useI18n } from "./i18n";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

function ErrorFallback({
  error,
  onRetry,
}: {
  error: Error;
  onRetry: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="launcher">
      <div className="crash">
        <h1>{t("Something went wrong on this screen")}</h1>
        <p>
          {t("Beta Start is still running. Reload the interface to return to the launcher.")}
        </p>
        <code className="crash-detail">{error.message}</code>
        <div className="settings-actions">
          <button
            className="primary"
            onClick={() => window.location.reload()}
          >
            {t("Reload")}
          </button>
          <button className="secondary" onClick={onRetry}>
            {t("Try again")}
          </button>
        </div>
        <p className="field-hint">
          {t("If the problem continues, open an issue with the message above.")}
        </p>
      </div>
    </div>
  );
}

/**
 * Prevents a render error from leaving a blank window (and the app looking
 * closed). Shows the error and lets the user reload the interface.
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
      <ErrorFallback
        error={this.state.error}
        onRetry={() => this.setState({ error: null })}
      />
    );
  }
}
