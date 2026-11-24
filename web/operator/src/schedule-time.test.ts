// @ts-nocheck
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  defaultStationScheduleInput,
  stationScheduleZoneIsValid,
  stationScheduleInput,
  stationScheduleInstant,
  formatStationScheduleTime,
} from './schedule-time.ts';

const DENVER = 'America/Denver';
const NEW_YORK = 'America/New_York';

test('zone validity', () => {
  assert.equal(stationScheduleZoneIsValid(DENVER), true);
  assert.equal(stationScheduleZoneIsValid(NEW_YORK), true);
  assert.equal(stationScheduleZoneIsValid('Mars/Olympus'), false);
  assert.equal(stationScheduleZoneIsValid(''), false);
});

test('same wall time maps to distinct known UTC instants', () => {
  assert.equal(stationScheduleInstant('2024-01-15T12:00', DENVER), Date.UTC(2024, 0, 15, 19));
  assert.equal(stationScheduleInstant('2024-01-15T12:00', NEW_YORK), Date.UTC(2024, 0, 15, 17));
});

test('station-wall defaults around UTC midnight', () => {
  assert.equal(stationScheduleInput(0, DENVER), '1969-12-31T17:00');
  assert.equal(stationScheduleInput(0, NEW_YORK), '1969-12-31T19:00');
});

test('spring gap rejected for Denver and New York', () => {
  for (const zone of [DENVER, NEW_YORK]) {
    assert.throws(() => stationScheduleInstant('2024-03-10T02:30', zone), /gap|daylight|does not exist/i);
    assert.equal(typeof stationScheduleInstant('2024-03-10T01:59', zone), 'number');
    assert.equal(typeof stationScheduleInstant('2024-03-10T03:00', zone), 'number');
  }
});

test('fall fold rejected for Denver and New York', () => {
  for (const zone of [DENVER, NEW_YORK]) {
    assert.throws(() => stationScheduleInstant('2024-11-03T01:30', zone), /ambiguous|twice|repeated/i);
    assert.equal(typeof stationScheduleInstant('2024-11-03T00:59', zone), 'number');
    assert.equal(typeof stationScheduleInstant('2024-11-03T02:00', zone), 'number');
  }
});

test('invalid date and zone throw readable errors', () => {
  assert.throws(() => stationScheduleInstant('not-a-date', DENVER), /invalid|date/i);
  assert.throws(() => stationScheduleInstant('2024-01-01T00:00', 'Mars/Olympus'), /zone|invalid/i);
});

test('format shows clock and short zone without locale assumptions', () => {
  const summer = Date.UTC(2024, 6, 1, 18);
  const winter = Date.UTC(2024, 0, 1, 18);
  const mdt = formatStationScheduleTime(summer, DENVER);
  const mst = formatStationScheduleTime(winter, DENVER);
  assert.match(mdt, /\bMDT\b/);
  assert.match(mst, /\bMST\b/);
  assert.match(mdt, /\d{1,2}:\d{2}/);
  assert.match(stationScheduleInput(summer, NEW_YORK), /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/);
});

test("defaultStationScheduleInput handles DST and UTC rollover", () => {
  assert.equal(defaultStationScheduleInput(Date.UTC(2024, 10, 3, 7, 29), DENVER), "");
  assert.equal(defaultStationScheduleInput(Date.UTC(2024, 10, 3, 8, 29), DENVER), "2024-11-03T02:30");
  assert.equal(defaultStationScheduleInput(Date.UTC(2024, 0, 1, 0, 0), NEW_YORK), "2023-12-31T19:01");
  assert.equal(defaultStationScheduleInput(Date.UTC(2024, 0, 1, 0, 0), "Invalid/Zone"), "");
});
