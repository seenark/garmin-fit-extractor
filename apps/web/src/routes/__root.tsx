import { Link, Outlet, createRootRoute } from "@tanstack/react-router";

export const Route = createRootRoute({
  component: RootLayout,
  errorComponent: ({ error }) => (
    <main className="shell"><section className="card error"><h1>Something went wrong</h1><p>{error instanceof Error ? error.message : "The requested page could not be loaded."}</p><Link to="/">Return to upload</Link></section></main>
  ),
});

function RootLayout() {
  return (
    <div className="shell">
      <header className="site-header">
        <Link className="brand" to="/">Garmin FIT Extractor</Link>
        <nav aria-label="Main navigation"><Link to="/" activeProps={{ "aria-current": "page" }}>Upload</Link><Link to="/history" search={{ offset: 0 }} activeProps={{ "aria-current": "page" }}>History</Link></nav>
      </header>
      <main className="page"><Outlet /></main>
    </div>
  );
}
