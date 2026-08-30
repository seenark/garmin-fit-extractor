import type { Analysis, Metric } from "../lib/api-types";
import {
  formatCalories,
  formatCadence,
  formatDate,
  formatDuration,
  formatHeartRate,
  formatHeartRateRange,
  formatLapHeartRate,
  formatMetric,
  formatPower,
  formatTemperature,
} from "../lib/formatters";

function MetricCard({ label, metric }: { label: string; metric: Metric }) {
  return (
    <div className="metric">
      <dt>{label}</dt>
      <dd>{formatMetric(metric)}</dd>
    </div>
  );
}

function ValueCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

export function AnalysisSummary({ analysis }: { analysis: Analysis }) {
  const cadence = analysis.runningDynamics.cadence;
  const activityType = analysis.activity.type ?? "Activity";

  return (
    <section aria-label="Activity analysis">
      <h2>{activityType} analysis</h2>
      <p className="muted">
        {analysis.source.fileName} · {formatDate(analysis.activity.date)}
      </p>

      <h3>Summary</h3>
      <dl className="grid">
        <MetricCard label="Duration" metric={analysis.summary.duration} />
        <MetricCard label="Moving time" metric={analysis.summary.movingTime} />
        <MetricCard label="Distance" metric={analysis.summary.distance} />
        <ValueCard
          label="Calories"
          value={formatCalories(analysis.summary.calories.value)}
        />
      </dl>

      <h3>Heart rate</h3>
      <dl className="grid">
        <ValueCard
          label="Average"
          value={formatHeartRate(analysis.heartRate.averageBpm)}
        />
        <ValueCard
          label="Maximum"
          value={formatHeartRate(analysis.heartRate.maximumBpm)}
        />
        {analysis.heartRate.zones.map((zone) => (
          <ValueCard
            key={zone.zone}
            label={`Zone ${zone.zone}`}
            value={`${formatHeartRateRange(zone.minBpm, zone.maxBpm)} · ${formatDuration(zone.durationSeconds)}`}
          />
        ))}
      </dl>

      <h3>Performance</h3>
      <dl className="grid">
        <MetricCard label="Average pace" metric={analysis.pace.average} />
        <MetricCard label="Moving pace" metric={analysis.pace.moving} />
        <MetricCard label="Best pace" metric={analysis.pace.best} />
        <ValueCard
          label="Average power"
          value={formatPower(analysis.power.averageWatts)}
        />
        <ValueCard
          label="Maximum power"
          value={formatPower(analysis.power.maximumWatts)}
        />
      </dl>

      <h3>Running dynamics</h3>
      <dl className="grid">
        <ValueCard
          label="Average cadence"
          value={formatCadence(cadence.averageStepsPerMinute)}
        />
        <ValueCard
          label="Maximum cadence"
          value={formatCadence(cadence.maximumStepsPerMinute)}
        />
        <MetricCard
          label="Stride length"
          metric={analysis.runningDynamics.strideLength}
        />
        <MetricCard
          label="Ground contact time"
          metric={analysis.runningDynamics.groundContactTime}
        />
        <MetricCard
          label="Vertical oscillation"
          metric={analysis.runningDynamics.verticalOscillation}
        />
        <MetricCard
          label="Vertical ratio"
          metric={analysis.runningDynamics.verticalRatio}
        />
      </dl>

      <h3>Elevation and temperature</h3>
      <dl className="grid">
        <MetricCard label="Ascent" metric={analysis.elevation.ascent} />
        <MetricCard label="Descent" metric={analysis.elevation.descent} />
        <ValueCard
          label="Average temperature"
          value={formatTemperature(analysis.temperature.averageCelsius)}
        />
        <ValueCard
          label="Minimum temperature"
          value={formatTemperature(analysis.temperature.minimumCelsius)}
        />
        <ValueCard
          label="Maximum temperature"
          value={formatTemperature(analysis.temperature.maximumCelsius)}
        />
      </dl>

      <h3>Laps</h3>
      {analysis.laps.length === 0 ? (
        <p className="muted">No laps were recorded.</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th scope="col">Lap</th>
                <th scope="col">Distance</th>
                <th scope="col">Duration</th>
                <th scope="col">Pace</th>
                <th scope="col">Heart rate</th>
              </tr>
            </thead>
            <tbody>
              {analysis.laps.map((lap) => (
                <tr key={lap.index}>
                  <td data-label="Lap">{lap.index}</td>
                  <td data-label="Distance">{formatMetric(lap.distance)}</td>
                  <td data-label="Duration">{formatMetric(lap.duration)}</td>
                  <td data-label="Pace">{formatMetric(lap.pace)}</td>
                  <td data-label="Heart rate">
                    {formatLapHeartRate(
                      lap.heartRate.averageBpm,
                      lap.heartRate.maximumBpm,
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
