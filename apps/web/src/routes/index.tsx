import { Link, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({ component: HomePage });

function HomePage() {
  return (
    <div className="home-page">
      <section className="home-hero" aria-labelledby="home-title">
        <div className="home-hero-copy">
          <p className="home-kicker">A closer look at your Garmin data</p>
          <h1 id="home-title">Read the data your Garmin already captured.</h1>
          <p className="home-lede">
            Export a Garmin ZIP, open the detailed FIT data, and hand the raw
            JSON to ChatGPT or Claude when the built-in summary stops short.
          </p>
          <div className="home-actions">
            <Link
              className="button home-primary-action"
              data-testid="home-upload-cta"
              to="/upload"
            >
              Upload ZIP files
            </Link>
            <Link
              className="button secondary"
              data-testid="home-history-cta"
              to="/history"
              search={{ offset: 0, order: "desc" }}
            >
              View history
            </Link>
          </div>
          <p className="home-constraint-note">
            1–10 ZIP files per upload · 20 megabytes per file
          </p>
        </div>

        <figure
          className="home-workflow"
          data-testid="home-workflow"
          aria-labelledby="workflow-title"
        >
          <figcaption id="workflow-title" className="home-workflow-heading">
            From Garmin export to an AI-ready handoff
          </figcaption>
          <div className="workflow-board">
            <div className="workflow-node workflow-node--source">
              <span className="workflow-node-label">01 / Garmin export</span>
              <span className="workflow-file">
                <span className="workflow-file-mark" aria-hidden="true">
                  ZIP
                </span>
                <span>activity.zip</span>
              </span>
              <span className="workflow-node-note">The file you export</span>
            </div>

            <div className="workflow-connector" aria-hidden="true">
              <span />
            </div>

            <div className="workflow-node workflow-node--extract">
              <span className="workflow-node-label">02 / FIT extraction</span>
              <span className="workflow-file">
                <span className="workflow-file-mark" aria-hidden="true">
                  FIT
                </span>
                <span>activity.fit</span>
              </span>
              <span className="workflow-node-note">Detailed activity records</span>
            </div>

            <div className="workflow-branch" aria-hidden="true">
              <span />
              <span />
            </div>

            <div className="workflow-outputs">
              <div className="workflow-output workflow-output--analysis">
                <span className="workflow-output-label">Normalized analysis</span>
                <strong>Readable metrics</strong>
                <span>laps · pace · heart rate</span>
              </div>
              <div className="workflow-output workflow-output--raw">
                <span className="workflow-output-label">Raw JSON</span>
                <strong>Every field in view</strong>
                <span>ready to copy or download</span>
              </div>
            </div>

            <div className="workflow-handoff">
              <span className="workflow-node-label">03 / AI handoff</span>
              <div className="workflow-ai-list">
                <span>ChatGPT</span>
                <span>Claude</span>
              </div>
              <span className="workflow-node-note">Ask deeper questions of the data</span>
            </div>
          </div>
          <p className="home-workflow-caption">One export. Two ways to read it.</p>
        </figure>
      </section>

      <section className="home-section home-process" aria-labelledby="process-title">
        <div className="home-section-heading">
          <h2 id="process-title">Turn a Garmin export into something you can ask about.</h2>
          <p>
            The workflow stays simple: bring the archive, inspect what the watch
            recorded, then choose the view that fits your next question.
          </p>
        </div>

        <ol className="home-steps">
          <li className="home-step">
            <span className="home-step-number" aria-hidden="true">
              1.0
            </span>
            <div className="home-step-copy">
              <h3>Export once from Garmin.</h3>
              <p>
                Start with the ZIP file from Garmin’s website. Upload a single
                activity or a batch of files without changing the source export.
              </p>
            </div>
            <div className="home-step-proof" aria-label="Upload input">
              <span className="home-proof-label">Input</span>
              <code>activity.zip</code>
              <span>Garmin export</span>
            </div>
          </li>
          <li className="home-step">
            <span className="home-step-number" aria-hidden="true">
              2.0
            </span>
            <div className="home-step-copy">
              <h3>See more than the headline numbers.</h3>
              <p>
                Read a normalized view across the activity summary, laps, pace,
                heart rate, power, cadence, elevation, temperature, and calories.
              </p>
            </div>
            <div className="home-step-proof home-step-proof--metrics" aria-label="Normalized analysis fields">
              <span className="home-proof-label">Analysis</span>
              <span>summary</span>
              <span>laps</span>
              <span>running dynamics</span>
            </div>
          </li>
          <li className="home-step">
            <span className="home-step-number" aria-hidden="true">
              3.0
            </span>
            <div className="home-step-copy">
              <h3>Give the raw data a wider audience.</h3>
              <p>
                Open or download raw JSON, then paste it into ChatGPT or Claude
                for questions that need more context than a standard summary.
              </p>
            </div>
            <div className="home-step-proof home-step-proof--json" aria-label="Raw JSON handoff">
              <span className="home-proof-label">Handoff</span>
              <code>{'{ "activity": … }'}</code>
              <span>copy or download</span>
            </div>
          </li>
        </ol>
      </section>

      <section className="home-section home-depth" aria-labelledby="depth-title">
        <div className="home-depth-intro">
          <h2 id="depth-title">More detail, without losing the source.</h2>
          <p>
            Garmin records the detail. This app gives that detail a shape you can
            inspect, save, and carry into the tools you already use.
          </p>
        </div>
        <ul className="home-benefits">
          <li>
            <h3>Normalized first</h3>
            <p>
              Start with consistent labels and human-readable units instead of
              digging through an unstructured export.
            </p>
          </li>
          <li>
            <h3>Raw when it matters</h3>
            <p>
              Keep the raw JSON available when a deeper question needs the full
              record rather than a selected summary.
            </p>
          </li>
          <li>
            <h3>History when you return</h3>
            <p>
              Reopen saved extractions later, so the useful work does not end
              when the first question has been answered.
            </p>
          </li>
        </ul>
      </section>

      <section className="home-closing" aria-labelledby="closing-title">
        <div>
          <h2 id="closing-title">Start with the export you already have.</h2>
          <p>
            Upload the ZIP, choose your view, and take the data where your next
            question lives.
          </p>
        </div>
        <div className="home-actions home-closing-actions">
          <Link className="button" to="/upload">
            Upload ZIP files
          </Link>
          <Link
            className="button quiet"
            to="/history"
            search={{ offset: 0, order: "desc" }}
          >
            View history
          </Link>
        </div>
      </section>
    </div>
  );
}
