import { Component, type ErrorInfo, type ReactNode } from "react";

export class ErrorBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(_error: Error, _info: ErrorInfo) {
    /* Never log repository metadata or credentials. */
  }
  render() {
    return this.state.failed ? (
      <main className="simple-empty">
        <h1>The interface encountered a problem.</h1>
        <p>
          Operation artifacts are retained. Reload to recover the durable
          operation state.
        </p>
        <button
          className="button button-primary"
          onClick={() => location.reload()}
        >
          Reload un-git
        </button>
      </main>
    ) : (
      this.props.children
    );
  }
}
