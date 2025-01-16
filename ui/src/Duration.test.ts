import { describe, expect, it } from "vitest";
import { elapsedString, parseDurationString } from "./Duration";

describe("durationParsing", () => {
	it("parses hours and minutes", () => {
		expect(parseDurationString("1h12m")).toEqual(1 * 3600 + 12 * 60);
	});
	it("parses minutes only", () => {
		expect(parseDurationString("900m")).toEqual(900 * 60);
	});
	it("parses just number (as minutes)", () => {
		expect(parseDurationString("69")).toEqual(69 * 60);
	});
	it("parses hours only", () => {
		expect(parseDurationString("2h")).toEqual(2 * 3600);
	});
});

describe("elapsedString", () => {
	it("works with five minutes", () =>
		expect(elapsedString(new Date(0), new Date(5 * 60 * 1000))).toEqual("5m"));

	it("works with full hours", () =>
		expect(elapsedString(new Date(0), new Date(5 * 3600 * 1000))).toEqual(
			"5h0m",
		));

	it("works with hours and minutes", () =>
		expect(
			elapsedString(new Date(0), new Date((5 * 60 + 5 * 3600) * 1000)),
		).toEqual("5h5m"));
});
