import type { Analysis, Metric } from "../lib/api-types";
import { ActivityCharts } from "./activity-charts";
import {
  formatCalories,
  formatCadence,
  formatDate,
  formatDuration,
  formatActivityType,
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
  const activityType = formatActivityType(analysis.activity.type);

  return (
    <section aria-label="การวิเคราะห์กิจกรรม">
      <h2>ผลวิเคราะห์ · {activityType}</h2>
      <p className="muted">
        {analysis.source.fileName} · {formatDate(analysis.activity.date)}
      </p>

      <h3>สรุป</h3>
      <dl className="grid">
        <MetricCard label="ระยะเวลา" metric={analysis.summary.duration} />
        <MetricCard label="เวลาที่เคลื่อนไหว" metric={analysis.summary.movingTime} />
        <MetricCard label="ระยะทาง" metric={analysis.summary.distance} />
        <ValueCard
          label="แคลอรี"
          value={formatCalories(analysis.summary.calories.value)}
        />
      </dl>

      <ActivityCharts analysis={analysis} />

      <h3>อัตราการเต้นหัวใจ</h3>
      <dl className="grid">
        <ValueCard
          label="ค่าเฉลี่ย"
          value={formatHeartRate(analysis.heartRate.averageBpm)}
        />
        <ValueCard
          label="ค่าสูงสุด"
          value={formatHeartRate(analysis.heartRate.maximumBpm)}
        />
        {analysis.heartRate.zones.map((zone) => (
          <ValueCard
            key={zone.zone}
            label={`โซน ${zone.zone}`}
            value={`${formatHeartRateRange(zone.minBpm, zone.maxBpm)} · ${formatDuration(zone.durationSeconds)}`}
          />
        ))}
      </dl>

      <h3>ประสิทธิภาพ</h3>
      <dl className="grid">
        <MetricCard label="เพซเฉลี่ย" metric={analysis.pace.average} />
        <MetricCard label="เพซขณะเคลื่อนไหว" metric={analysis.pace.moving} />
        <MetricCard label="เพซดีที่สุด" metric={analysis.pace.best} />
        <ValueCard
          label="กำลังเฉลี่ย"
          value={formatPower(analysis.power.averageWatts)}
        />
        <ValueCard
          label="กำลังสูงสุด"
          value={formatPower(analysis.power.maximumWatts)}
        />
      </dl>

      <h3>ไดนามิกการวิ่ง (Running dynamics)</h3>
      <dl className="grid">
        <ValueCard
          label="รอบขาเฉลี่ย"
          value={formatCadence(cadence.averageStepsPerMinute)}
        />
        <ValueCard
          label="รอบขาสูงสุด"
          value={formatCadence(cadence.maximumStepsPerMinute)}
        />
        <MetricCard
          label="ความยาวก้าว"
          metric={analysis.runningDynamics.strideLength}
        />
        <MetricCard
          label="เวลาสัมผัสพื้น"
          metric={analysis.runningDynamics.groundContactTime}
        />
        <MetricCard
          label="การแกว่งตัวแนวตั้ง"
          metric={analysis.runningDynamics.verticalOscillation}
        />
        <MetricCard
          label="สัดส่วนแนวตั้ง"
          metric={analysis.runningDynamics.verticalRatio}
        />
      </dl>

      <h3>ระดับความสูงและอุณหภูมิ</h3>
      <dl className="grid">
        <MetricCard label="ไต่ระดับขึ้น" metric={analysis.elevation.ascent} />
        <MetricCard label="ไต่ระดับลง" metric={analysis.elevation.descent} />
        <ValueCard
          label="อุณหภูมิเฉลี่ย"
          value={formatTemperature(analysis.temperature.averageCelsius)}
        />
        <ValueCard
          label="อุณหภูมิต่ำสุด"
          value={formatTemperature(analysis.temperature.minimumCelsius)}
        />
        <ValueCard
          label="อุณหภูมิสูงสุด"
          value={formatTemperature(analysis.temperature.maximumCelsius)}
        />
      </dl>

      <h3>รอบ</h3>
      {analysis.laps.length === 0 ? (
        <p className="muted">ไม่มีข้อมูลรอบ</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th scope="col">รอบ</th>
                <th scope="col">ระยะทาง</th>
                <th scope="col">ระยะเวลา</th>
                <th scope="col">เพซ</th>
                <th scope="col">อัตราการเต้นหัวใจ</th>
              </tr>
            </thead>
            <tbody>
              {analysis.laps.map((lap) => (
                <tr key={lap.index}>
                  <td data-label="รอบ">{lap.index}</td>
                  <td data-label="ระยะทาง">{formatMetric(lap.distance)}</td>
                  <td data-label="ระยะเวลา">{formatMetric(lap.duration)}</td>
                  <td data-label="เพซ">{formatMetric(lap.pace)}</td>
                  <td data-label="อัตราการเต้นหัวใจ">
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
