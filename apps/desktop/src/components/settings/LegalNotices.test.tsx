// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import {
	LegalNotices,
	LICENSE_URL,
	NOTICE_URL,
	REPO_URL,
	THIRD_PARTY_URL,
	sourceUrl,
} from "./LegalNotices";
import { openUrl } from "@/lib/openUrl";

vi.mock("@/lib/openUrl", () => ({ openUrl: vi.fn() }));

describe("LegalNotices", () => {
	afterEach(() => {
		cleanup();
		vi.clearAllMocks();
	});

	it("shows the author, copyright, license and lack of warranty", () => {
		render(<LegalNotices version="1.2.3" />);
		expect(screen.getByText("Created by Max Boksem")).toBeTruthy();
		expect(screen.getByText("Copyright © 2026 Max Boksem")).toBeTruthy();
		expect(screen.getByText(/GNU General Public License version 3/)).toBeTruthy();
		expect(screen.getByText(/It comes with no warranty/)).toBeTruthy();
	});

	it.each([
		["License", LICENSE_URL],
		["Additional terms", NOTICE_URL],
		["Third-party notices", THIRD_PARTY_URL],
	])("opens %s in the browser", (label, url) => {
		render(<LegalNotices version="1.2.3" />);
		fireEvent.click(screen.getByRole("button", { name: label }));
		expect(openUrl).toHaveBeenCalledWith(url);
	});

	it("links to the source of the running version", () => {
		render(<LegalNotices version="1.2.3" />);
		fireEvent.click(screen.getByRole("button", { name: "Source code" }));
		expect(openUrl).toHaveBeenCalledWith(`${REPO_URL}/tree/v1.2.3`);
	});

	it("links to the repository while the version is unknown", () => {
		expect(sourceUrl(undefined)).toBe(REPO_URL);
		expect(sourceUrl("")).toBe(REPO_URL);
	});
});
