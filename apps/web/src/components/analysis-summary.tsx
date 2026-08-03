import type { Analysis, Metric } from "../lib/api-types";

function display(value: number | string | null): string {
  return value === null ? "—" : String(value);
}

function MetricCard({ label, metric }: { label: string; metric: Metric }) {
  return <div className="metric"><dt>{label}</dt><dd>{display(metric.value)} {metric.unit}</dd></div>;
}

export function AnalysisSummary({ analysis }: { analysis: Analysis }) {
  const cadence = analysis.runningDynamics.cadence;
  return (
    <section aria-label="Normalized analysis">
      <h2>{display(analysis.activity.type)} activity</h2>
      <p className="muted">{analysis.source.fileName} · {display(analysis.activity.date)}</p>
      <h3>Summary</h3>
      <dl className="grid">
        <MetricCard label="Duration" metric={analysis.summary.duration} />
        <MetricCard label="Moving time" metric={analysis.summary.movingTime} />
        <MetricCard label="Distance" metric={analysis.summary.distance} />
        <div className="metric"><dt>Calories</dt><dd>{display(analysis.summary.calories.value)} {analysis.summary.calories.unit}</dd></div>
      </dl>
      <h3>Heart rate</h3>
      <dl className="grid">
        <div className="metric"><dt>Average</dt><dd>{display(analysis.heartRate.averageBpm)} bpm</dd></div>
        <div className="metric"><dt>Maximum</dt><dd>{display(analysis.heartRate.maximumBpm)} bpm</dd></div>
        {analysis.heartRate.zones.map((zone) => <div className="metric" key={zone.zone}><dt>Zone {zone.zone}</dt><dd>{display(zone.minBpm)}–{display(zone.maxBpm)} bpm · {display(zone.durationSeconds)} s</dd></div>)}
      </dl>
      <h3>Performance</h3>
      <dl className="grid">
        <MetricCard label="Average pace" metric={analysis.pace.average} />
        <MetricCard label="Moving pace" metric={analysis.pace.moving} />
        <MetricCard label="Best pace" metric={analysis.pace.best} />
        <div className="metric"><dt>Average power</dt><dd>{display(analysis.power.averageWatts)} W</dd></div>
        <div className="metric"><dt>Maximum power</dt><dd>{display(analysis.power.maximumWatts)} W</dd></div>
      </dl>
      <h3>Running dynamics</h3>
      <dl className="grid">
        <div className="metric"><dt>Average cadence</dt><dd>{display(cadence.averageStepsPerMinute)} spm</dd></div>
        <div className="metric"><dt>Maximum cadence</dt><dd>{display(cadence.maximumStepsPerMinute)} spm</dd></div>
        <MetricCard label="Stride length" metric={analysis.runningDynamics.strideLength} />
        <MetricCard label="Ground contact time" metric={analysis.runningDynamics.groundContactTime} />
        <MetricCard label="Vertical oscillation" metric={analysis.runningDynamics.verticalOscillation} />
        <MetricCard label="Vertical ratio" metric={analysis.runningDynamics.verticalRatio} />
      </dl>
      <h3>Elevation and temperature</h3>
      <dl className="grid">
        <MetricCard label="Ascent" metric={analysis.elevation.ascent} />
        <MetricCard label="Descent" metric={analysis.elevation.descent} />
        <div className="metric"><dt>Average temperature</dt><dd>{display(analysis.temperature.averageCelsius)} °C</dd></div>
        <div className="metric"><dt>Minimum temperature</dt><dd>{display(analysis.temperature.minimumCelsius)} °C</dd></div>
        <div className="metric"><dt>Maximum temperature</dt><dd>{display(analysis.temperature.maximumCelsius)} °C</dd></div>
      </dl>
      <h3>Laps</h3>
      {analysis.laps.length === 0 ? <p className="muted">No laps were recorded.</p> : <div className="table-wrap"><table><thead><tr><th>Lap</th><th>Distance</th><th>Duration</th><th>Pace</th><th>Heart rate</th></tr></thead><tbody>{analysis.laps.map((lap) => <tr key={lap.index}><td>{lap.index}</td><td>{display(lap.distance.value)} {lap.distance.unit}</td><td>{display(lap.duration.value)} {lap.duration.unit}</td><td>{display(lap.pace.value)} {lap.pace.unit}</td><td>{display(lap.heartRate.averageBpm)} / {display(lap.heartRate.maximumBpm)} bpm</td></tr>)}</tbody></table></div>}
    </section>
  );
}
