import React from "react";

export const FILTER_COLUMN_VIEWPORT_HEIGHT_PX = 60;

type FilterColumnViewportProps = {
	children: React.ReactNode;
	onMouseLeave?: React.MouseEventHandler<HTMLDivElement>;
};

export const FilterColumnViewport = React.forwardRef<HTMLDivElement, FilterColumnViewportProps>(
	function FilterColumnViewport({ children, onMouseLeave }, ref) {
		return (
			<div
				ref={ref}
				data-testid="timeline-filter-column-viewport"
				className="absolute left-3 z-30 flex flex-col gap-0.5 items-start overscroll-contain scrollbar-hide"
				style={{ top: 0, maxHeight: FILTER_COLUMN_VIEWPORT_HEIGHT_PX, overflowY: "auto" }}
				dir="ltr"
				onMouseLeave={onMouseLeave}
			>
				{children}
			</div>
		);
	},
);

FilterColumnViewport.displayName = "FilterColumnViewport";
