import { render, fireEvent, act, cleanup } from "@testing-library/react";
import { afterEach, describe, it, expect, vi } from "vitest";
import type { Channel } from "@/lib/types";
import { ChannelOverlay } from "./ChannelOverlay";

const ch = (
	id: string,
	name: string,
	contentType: Channel["contentType"],
	groupTitle = "Group"
): Channel => ({
	id,
	name,
	url: `http://h/${id}`,
	groupTitle,
	isFavorite: false,
	contentType,
	sources: [],
});

const channels = [
	ch("l1", "BBC One", "live", "UK"),
	ch("l2", "CNN", "live", "News"),
	ch("m1", "Dune", "movie"),
	ch("s1", "Homeland S01E01", "series"),
];

vi.mock("@/hooks/useChannels", () => ({
	useChannels: () => ({ channels }),
}));

// jsdom has no layout, so the real virtualizer would render nothing.
vi.mock("@tanstack/react-virtual", () => ({
	useVirtualizer: ({ count }: { count: number }) => ({
		getTotalSize: () => count * 48,
		getVirtualItems: () =>
			Array.from({ length: count }, (_, index) => ({ index, start: index * 48 })),
	}),
}));

const names = (container: HTMLElement) =>
	Array.from(container.querySelectorAll("p.text-xs.font-medium")).map((p) => p.textContent);

describe("ChannelOverlay", () => {
	afterEach(cleanup);

	it("lists only live channels", () => {
		const { container } = render(
			<ChannelOverlay onClose={vi.fn()} onSelectChannel={vi.fn()} />
		);
		expect(names(container)).toEqual(["BBC One", "CNN"]);
	});

	it("filters by name or group", async () => {
		const { container, getByLabelText } = render(
			<ChannelOverlay onClose={vi.fn()} onSelectChannel={vi.fn()} />
		);
		await act(async () => {
			fireEvent.change(getByLabelText("Filter channels"), { target: { value: "news" } });
		});
		expect(names(container)).toEqual(["CNN"]);
	});

	it("selecting a channel plays it and closes the overlay", () => {
		const onClose = vi.fn();
		const onSelect = vi.fn();
		const { getByText } = render(
			<ChannelOverlay onClose={onClose} onSelectChannel={onSelect} />
		);
		fireEvent.click(getByText("BBC One"));
		expect(onSelect).toHaveBeenCalledWith(channels[0]);
		expect(onClose).toHaveBeenCalledOnce();
	});

	it("Escape in the filter clears it first, then closes", async () => {
		const onClose = vi.fn();
		const { getByLabelText } = render(
			<ChannelOverlay onClose={onClose} onSelectChannel={vi.fn()} />
		);
		const input = getByLabelText("Filter channels") as HTMLInputElement;
		await act(async () => {
			fireEvent.change(input, { target: { value: "bbc" } });
		});
		fireEvent.keyDown(input, { key: "Escape" });
		expect(input.value).toBe("");
		expect(onClose).not.toHaveBeenCalled();
		fireEvent.keyDown(input, { key: "Escape" });
		expect(onClose).toHaveBeenCalledOnce();
	});
});
