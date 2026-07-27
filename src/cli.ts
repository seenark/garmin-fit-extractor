#!/usr/bin/env node

import { access, writeFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { Command } from "commander";
import { analyzeFitFile } from "./analyze.js";

const program = new Command();

program
  .name("garmin-coach")
  .description("Convert Garmin FIT activity files into stable JSON for AI coaches.")
  .version("0.1.0");

program
  .command("analyze")
  .description("Analyze one FIT activity file")
  .argument("<file>", "path to a Garmin FIT file")
  .option("-o, --output <file>", "output JSON file", "analysis.json")
  .action(async (input: string, options: { output: string }) => {
    try {
      const inputPath = resolve(input);
      const outputPath = resolve(options.output);

      if (extname(inputPath).toLowerCase() !== ".fit") {
        throw new Error("Input file must use the .fit extension.");
      }

      await access(inputPath);
      const analysis = await analyzeFitFile(inputPath);
      await writeFile(outputPath, `${JSON.stringify(analysis, null, 2)}\n`, "utf8");
      process.stdout.write(`${outputPath}\n`);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      process.stderr.write(`garmin-coach: ${message}\n`);
      process.exitCode = 1;
    }
  });

await program.parseAsync(process.argv);
