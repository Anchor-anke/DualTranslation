// @vitest-environment node
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import Ajv2020, { type AnySchema } from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import { describe, expect, it } from "vitest";

const root = fileURLToPath(new URL("..", import.meta.url));

const contracts = [
  "canonical-task-spec",
  "explanation-spec",
  "provider-profile",
  "conversion-request",
  "conversion-response",
] as const;

async function readJson(path: string): Promise<unknown> {
  return JSON.parse(await readFile(`${root}/${path}`, "utf8")) as unknown;
}

describe("JSON Schema contracts", () => {
  it.each(contracts)("accepts valid %s and rejects its invalid fixture", async (name) => {
    const schemas = await Promise.all(
      contracts.map(async (schemaName) => readJson(`schemas/${schemaName}.schema.json`)),
    );
    const ajv = new Ajv2020({ allErrors: true, strict: true });
    addFormats(ajv);
    for (const schema of schemas) {
      ajv.addSchema(schema as AnySchema);
    }

    const schema = schemas[contracts.indexOf(name)] as { $id: string };
    const validate = ajv.getSchema(schema.$id);
    expect(validate).toBeDefined();

    const validFixture = await readJson(`tests/fixtures/${name}.valid.json`);
    const invalidFixture = await readJson(`tests/fixtures/${name}.invalid.json`);

    expect(validate?.(validFixture), JSON.stringify(validate?.errors)).toBe(true);
    expect(validate?.(invalidFixture)).toBe(false);
  });
});
