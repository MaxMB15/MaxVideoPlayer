import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { CategoryBrowser } from "./CategoryBrowser";

describe("CategoryBrowser", () => {
	const defaultProps = {
		superCategories: [{ name: "United States", groupCount: 3, channelCount: 150 }],
		topLevelGroups: [{ name: "Misc", channelCount: 10 }],
		onSelectCategory: vi.fn(),
		onSelectGroup: vi.fn(),
		onManage: vi.fn(),
	};

	it("renders super-categories and top-level groups", () => {
		render(<CategoryBrowser {...defaultProps} />);
		expect(screen.getByText("United States")).toBeDefined();
		expect(screen.getByText("Misc")).toBeDefined();
		expect(screen.getByText("Manage")).toBeDefined();
	});

	it("calls onSelectCategory when clicking a super-category", () => {
		render(<CategoryBrowser {...defaultProps} />);
		fireEvent.click(screen.getAllByText("United States")[0]);
		expect(defaultProps.onSelectCategory).toHaveBeenCalledWith("United States");
	});
});
