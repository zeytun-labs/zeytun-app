import { expect, test } from "vitest";
import { defaults } from "sveltekit-superforms";
import { zod4, zod4Client } from "sveltekit-superforms/adapters";
import { profileFormSchema } from "./profile-schema";
import { proxyFormSchema } from "./proxy-schema";
import { ruleFormSchema } from "./rule-schema";

// Exercise the upstream adapter through the same narrowed Vite entry as the UI.
test("profile validation retains transforms in full and client adapters", async () => {
  for (const adapter of [zod4(profileFormSchema), zod4Client(profileFormSchema)]) {
    expect(await adapter.validate({ name: "  Work  ", icon: null })).toEqual({
      success: true,
      data: { name: "Work", icon: null },
    });
  }
});

test("rule validation still reports required outbound errors", async () => {
  const result = await zod4(ruleFormSchema).validate({
    kind: "DOMAIN-SUFFIX",
    value: "example.com",
    outbound: "",
  });
  expect(result.success).toBe(false);
  if (!result.success) {
    expect(result.issues).toContainEqual({
      path: ["outbound"],
      message: "Outbound is required",
    });
  }
});

test("proxy form initialization retains nested defaults and port constraints", () => {
  const form = defaults(zod4(proxyFormSchema));
  expect(form.data.settings.tls).toMatchObject({
    enabled: false,
    allow_insecure: false,
  });
  expect(form.data.settings.multiplex.enabled).toBe(false);
  expect(form.constraints.port).toMatchObject({ min: 1, max: 65535 });
});
