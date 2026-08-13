import { Link, Outlet, createRootRoute, useRouter } from "@tanstack/react-router";
import { useState } from "react";
import { ApiError, getCurrentUser, logout, startGoogleLogin } from "../lib/api";

type RootSearch = { authError?: "AUTH_FAILED" };

export const Route = createRootRoute({
  validateSearch: (search: Record<string, unknown>): RootSearch => ({
    authError:
      search.authError === "AUTH_FAILED" ? "AUTH_FAILED" : undefined,
  }),
  loader: async () => {
    try {
      return await getCurrentUser();
    } catch (error) {
      if (error instanceof ApiError && error.code === "AUTH_REQUIRED") {
        return { user: null };
      }
      throw error;
    }
  },
  component: RootLayout,
  errorComponent: ({ error }) =>
    error instanceof ApiError && error.code === "AUTH_REQUIRED" ? (
      <SignInScreen />
    ) : (
      <main className="shell">
        <section className="card error">
          <h1>Something went wrong</h1>
          <p>
            {error instanceof Error
              ? error.message
              : "The requested page could not be loaded."}
          </p>
          <Link to="/">Return to upload</Link>
        </section>
      </main>
    ),
});
function SignInScreen({ authError = false }: { authError?: boolean }) {
  return (
    <main className="shell">
      <section className="card">
        <h1>Garmin FIT Extractor</h1>
        <p>Sign in with Google to continue.</p>
        {authError ? (
          <p className="error" role="alert">
            Google sign-in failed. Try again.
          </p>
        ) : null}
        <button type="button" onClick={startGoogleLogin}>
          Continue with Google
        </button>
      </section>
    </main>
  );
}
function RootLayout() {
  const user = Route.useLoaderData().user;
  const search = Route.useSearch();
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  if (!user) {
    return <SignInScreen authError={search.authError === "AUTH_FAILED"} />;
  }
  return <div className="shell"><header className="site-header"><Link className="brand" to="/">Garmin FIT Extractor</Link><nav aria-label="Main navigation"><Link to="/" activeProps={{"aria-current":"page"}}>Upload</Link><Link to="/history" search={{offset:0,order:"desc"}} activeProps={{"aria-current":"page"}}>History</Link></nav><span>{user.displayName ?? user.email}</span><button type="button" disabled={busy} onClick={async()=>{setBusy(true);try{await logout();await router.invalidate();}finally{setBusy(false);}}}>Sign out</button></header><main className="page"><Outlet /></main></div>;
}
