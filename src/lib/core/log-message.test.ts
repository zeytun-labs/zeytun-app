import { describe, expect, it } from "vitest";
import { stripTerminalControls } from "./log-message";

describe("stripTerminalControls", () => {
  it("removes the core's terminal color codes while retaining its message", () => {
    expect(stripTerminalControls(
      "\u001b[36mINFO\u001b[0m[0000] service/api[0]: tcp server started at 127.0.0.1:19999",
    )).toBe("INFO[0000] service/api[0]: tcp server started at 127.0.0.1:19999");
  });

  it("handles extended colors and cursor commands", () => {
    expect(stripTerminalControls("\u001b[2K\u001b[38;2;20;40;60mconnected\u009b0m"))
      .toBe("connected");
  });

  it("preserves visible text inside terminal hyperlinks", () => {
    expect(stripTerminalControls("Open \u001b]8;;https://example.test\u001b\\details\u001b]8;;\u001b\\"))
      .toBe("Open details");
    expect(stripTerminalControls("\u001b]0;terminal title\u0007ready"))
      .toBe("ready");
  });

  it("preserves ordinary brackets, Unicode, tabs, and multiline messages", () => {
    const message = "[INFO] dns[0]: پاسخ آماده است ✓\n\t[2001:db8::1]:443";
    expect(stripTerminalControls(message)).toBe(message);
  });
});
