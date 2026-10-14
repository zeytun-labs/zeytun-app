import { afterEach, expect, test, vi } from "vitest";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import {
  ConnectionEventsSchema,
  SubscribeConnectionsRequestSchema,
} from "../gen/started_service_pb";
import { GrpcWebSocketStream } from "./websocket";

class MockSocket {
  static OPEN = 1;
  static latest: MockSocket;
  readyState = MockSocket.OPEN;
  binaryType = "";
  onopen!: () => void;
  onmessage!: (event: { data: ArrayBufferLike }) => void;
  onerror!: () => void;
  onclose!: (event: { reason: string }) => void;
  send = vi.fn();
  close = vi.fn();

  constructor(readonly url: string, readonly protocols: string[]) {
    MockSocket.latest = this;
  }
}

function setup() {
  vi.stubGlobal("WebSocket", MockSocket);
  const onEnd = vi.fn();
  const onMessage = vi.fn();
  const stream = new GrpcWebSocketStream({
    config: { url: "http://127.0.0.1:9999", secret: "test" },
    service: "daemon.StartedService",
    method: "SubscribeConnections",
    requestSchema: SubscribeConnectionsRequestSchema,
    responseSchema: ConnectionEventsSchema,
    onEnd,
    onMessage,
  });
  return { stream, socket: MockSocket.latest, onEnd, onMessage };
}

function frame(flag: number, body: Uint8Array) {
  const result = new Uint8Array(5 + body.length);
  result[0] = flag;
  new DataView(result.buffer).setUint32(1, body.length);
  result.set(body, 5);
  return result;
}

const trailer = (status = 0) =>
  frame(0x80, new TextEncoder().encode(`grpc-status: ${status}\r\n`));
const reply = () =>
  frame(0, toBinary(ConnectionEventsSchema, create(ConnectionEventsSchema, { reset: true })));

function combined(...frames: Uint8Array[]) {
  const result = new Uint8Array(frames.reduce((n, next) => n + next.length, 0));
  let offset = 0;
  for (const next of frames) {
    result.set(next, offset);
    offset += next.length;
  }
  return result;
}

afterEach(() => vi.unstubAllGlobals());

test("terminal trailers stop delivery within a coalesced WebSocket message", () => {
  const { socket, onEnd, onMessage } = setup();
  socket.onmessage({ data: combined(trailer(), reply()).buffer });
  socket.onmessage({ data: reply().buffer });
  socket.onclose({ reason: "" });
  expect(onEnd).toHaveBeenCalledTimes(1);
  expect(onMessage).not.toHaveBeenCalled();
});

test("protobuf schemas encode queued requests and decode fragmented replies", () => {
  const { stream, socket, onEnd, onMessage } = setup();
  stream.send({ interval: 1_000_000_000n });
  stream.finishSend();
  socket.onopen();
  expect(socket.protocols).toEqual(["grpc-websockets"]);
  expect(new TextDecoder().decode(socket.send.mock.calls[0][0]))
    .toContain("authorization: Bearer test");
  const sent = socket.send.mock.calls[1][0] as Uint8Array;
  expect(Array.from(sent.slice(0, 2))).toEqual([0, 0]);
  expect(new DataView(sent.buffer).getUint32(2)).toBe(sent.length - 6);
  expect(fromBinary(SubscribeConnectionsRequestSchema, sent.slice(6)).interval)
    .toBe(1_000_000_000n);
  expect(socket.send.mock.calls[2][0]).toEqual(new Uint8Array([1]));
  const data = reply();
  socket.onmessage({ data: data.slice(0, 3).buffer });
  expect(onMessage).not.toHaveBeenCalled();
  socket.onmessage({ data: data.slice(3).buffer });
  socket.onmessage({ data: trailer().buffer });
  expect(onMessage).toHaveBeenCalledTimes(1);
  expect(onMessage.mock.calls[0][0].reset).toBe(true);
  expect(onEnd).toHaveBeenCalledWith({ code: 0, message: "" }, undefined);
});

test("initial metadata and data precede terminal error status in one chunk", () => {
  const { socket, onEnd, onMessage } = setup();
  const headers = frame(0x80, new TextEncoder().encode("content-type: application/grpc\r\n"));
  socket.onmessage({ data: combined(headers, reply(), trailer(14), reply()).buffer });
  expect(onMessage).toHaveBeenCalledTimes(1);
  expect(onEnd).toHaveBeenCalledWith({ code: 14, message: "" }, undefined);
  expect(onMessage.mock.invocationCallOrder[0]).toBeLessThan(onEnd.mock.invocationCallOrder[0]);
});

test("closing before open discards queued requests and ignores late frames", () => {
  const { stream, socket, onEnd, onMessage } = setup();
  stream.send({ interval: 1n });
  stream.close();
  socket.onopen();
  stream.send({ interval: 2n });
  stream.finishSend();
  socket.onmessage({ data: reply().buffer });
  expect(socket.send).not.toHaveBeenCalled();
  expect(onEnd).not.toHaveBeenCalled();
  expect(onMessage).not.toHaveBeenCalled();
});

test("a decode failure ends the stream once and ignores subsequent frames", () => {
  const { socket, onEnd, onMessage } = setup();
  // Truncated length-delimited field.
  socket.onmessage({ data: frame(0, new Uint8Array([0x0a, 0xff])).buffer });
  socket.onmessage({ data: reply().buffer });
  socket.onerror();
  expect(onEnd).toHaveBeenCalledTimes(1);
  expect(onEnd).toHaveBeenCalledWith(null, expect.stringContaining("decode response"));
  expect(onMessage).not.toHaveBeenCalled();
});
