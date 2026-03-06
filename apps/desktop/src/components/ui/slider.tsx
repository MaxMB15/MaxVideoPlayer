import * as React from "react";
import { cn } from "@/lib/utils";

interface SliderProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange"> {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onValueChange?: (value: number) => void;
}

const Slider = React.forwardRef<HTMLInputElement, SliderProps>(
  ({ className, value, min = 0, max = 100, step = 1, onValueChange, ...props }, ref) => {
    const percent = ((value - min) / (max - min)) * 100;
    return (
      <input
        ref={ref}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onValueChange?.(Number(e.target.value))}
        className={cn(
          "w-full h-2 rounded-full appearance-none cursor-pointer bg-secondary",
          "[&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-primary",
          className
        )}
        style={{
          background: `linear-gradient(to right, hsl(var(--primary)) ${percent}%, hsl(var(--secondary)) ${percent}%)`,
        }}
        {...props}
      />
    );
  }
);
Slider.displayName = "Slider";

export { Slider };
