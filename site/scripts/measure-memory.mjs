// Linux RSS sampling. Pass only app/helper PIDs; this never reads VPN profiles.
// node scripts/measure-memory.mjs idle=123 active=456 engines=789,790
import { readFileSync } from 'node:fs';
import { setTimeout } from 'node:timers/promises';
const groups = Object.fromEntries(process.argv.slice(2).map(arg => {
  const [name, ids] = arg.split('=');
  if (!name || !/^\d+(,\d+)*$/.test(ids)) throw new Error('Use label=PID[,PID]');
  return [name, ids.split(',').map(Number)];
}));
if (!Object.keys(groups).length) throw new Error('Provide at least one PID.');
const rss = pid => {
  const status = readFileSync(`/proc/${pid}/status`, 'utf8');
  const match = status.match(/^VmRSS:\s+(\d+) kB$/m);
  if (!match) throw new Error(`RSS unavailable for PID ${pid}`);
  return Number(match[1]) / 1024;
};
const series = Object.fromEntries(Object.keys(groups).map(key => [key, []]));
const startedAt = new Date().toISOString();
for (let i = 0; i < 60; i++) {
  for (const [name, ids] of Object.entries(groups)) series[name].push(ids.reduce((sum, pid) => sum + rss(pid), 0));
  if (i < 59) await setTimeout(1000);
}
console.log(JSON.stringify({ startedAt, finishedAt: new Date().toISOString(), samples: 60, intervalMs: 1000, metric: 'RSS', unit: 'MiB', groups: Object.fromEntries(Object.entries(series).map(([name, values]) => [name, {
  mean: values.reduce((a, b) => a + b, 0) / values.length,
  min: Math.min(...values), max: Math.max(...values), values,
}])) }, null, 2));
