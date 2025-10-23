export type Duration = { secs: number; nanos: number };

export function serializeDurationSeconds(seconds: number): Duration {
  return { secs: seconds, nanos: 0 };
}

export function parseDurationString(duration: string): number {
  // Supported format: 130m or 1h30 or 1h30m
  const minutesOnlyRe = /^(\d+)m?$/;
  const hoursMinutesRe = /^(\d+)h(\d+)m?$/;
  const hoursOnlyRe = /^(\d)+h$/;

  const minutesOnlyMatch = duration.match(minutesOnlyRe);
  if (minutesOnlyMatch !== null) {
    return Number.parseInt(minutesOnlyMatch[1]) * 60;
  }

  const hoursMinutesMatch = duration.match(hoursMinutesRe);
  if (hoursMinutesMatch !== null) {
    return (
      Number.parseInt(hoursMinutesMatch[1]) * 3600 +
      Number.parseInt(hoursMinutesMatch[2]) * 60
    );
  }

  const hoursOnlyMatch = duration.match(hoursOnlyRe);
  if (hoursOnlyMatch !== null) {
    return Number.parseInt(hoursOnlyMatch[1]) * 3600;
  }

  throw Error(`Invalid duration ${duration}`);
}

export function isValidDurationString(duration: string | undefined): boolean {
  if (duration === undefined) {
    return false;
  }
  try {
    parseDurationString(duration);
  } catch (error) {
    return false;
  }
  return true;
}

export function elapsedString(start: Date, end: Date): string {
  const milliseconds = end.valueOf() - start.valueOf();
  return elapsedSecondsString(milliseconds / 1000);
}

export function elapsedSecondsString(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  if (minutes > 60) {
    const hours = Math.floor(minutes / 60);
    const minutes_rest = minutes % 60;

    return `${hours}h${minutes_rest}m`;
  }
  return `${minutes}m`;
}
