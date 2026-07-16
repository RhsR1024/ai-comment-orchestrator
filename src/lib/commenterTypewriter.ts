export const TYPEWRITER_FRAME_MS = 16;

export function advanceTypewriterText(current: string, target: string): string {
  if (current === target) {
    return current;
  }
  if (!target.startsWith(current)) {
    return target;
  }
  const backlog = target.length - current.length;
  const step = backlog > 1600 ? 16 : backlog > 800 ? 8 : backlog > 320 ? 4 : backlog > 80 ? 2 : 1;
  return target.slice(0, current.length + Math.min(backlog, step));
}
