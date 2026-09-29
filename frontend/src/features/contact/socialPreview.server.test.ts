import assert from "node:assert/strict";
import test from "node:test";
import { parseProfile } from "./socialPreview.server.ts";

test("社交账号链接按平台规范化", () => {
  assert.deepEqual(parseProfile("https://twitter.com/example_1?lang=zh"), {
    service: "x",
    handle: "example_1",
    url: "https://x.com/example_1",
  });
  assert.deepEqual(parseProfile("https://youtube.com/@example/"), {
    service: "youtube",
    handle: "@example",
    url: "https://www.youtube.com/@example",
  });
  assert.deepEqual(parseProfile("https://www.youtube.com/channel/UC123"), {
    service: "youtube",
    handle: "channel/UC123",
    url: "https://www.youtube.com/channel/UC123",
  });
  assert.equal(parseProfile("https://telegram.me/example")?.url, "https://t.me/example");
  assert.equal(parseProfile("https://github.com/example")?.service, "github");
});

test("拒绝任意域名、凭据、非主页路径和不安全协议", () => {
  for (const input of [
    "http://github.com/example",
    "https://github.com.evil.test/example",
    "https://user:pass@github.com/example",
    "https://github.com/example/repo",
    "https://youtube.com/redirect?to=http://127.0.0.1",
    "https://t.me/+invite",
    "https://x.com/example%2Fsecret",
  ]) {
    assert.equal(parseProfile(input), null, input);
  }
});
