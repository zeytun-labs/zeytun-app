/** Remove terminal formatting before a core message enters the UI log buffer. */
export function stripTerminalControls(message: string): string {
  return message
    // OSC strings include terminal titles and hyperlink targets (BEL or ST ends them).
    .replace(/(?:\u001b\]|\u009d)[^\u0007\u001b\u009c]*(?:\u0007|\u001b\\|\u009c)/g, "")
    // CSI strings include SGR colors, cursor movement, and erase commands.
    .replace(/(?:\u001b\[|\u009b)[0-?]*[ -/]*[@-~]/g, "")
    .replace(/\u001b[ -/]*[@-Z\\-_]/g, "");
}
