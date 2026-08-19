import React from "react";
import { BrandMark } from "./BrandMark";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./ui/dialog";
import { VisuallyHidden } from "./ui/visually-hidden";
import { About } from "./About";

interface LogoProps {
    isCollapsed: boolean;
}

/**
 * Expanded, the mark sits beside the wordmark; collapsed, it stands alone.
 *
 * The wordmark used to be a blue pill with grey text, which belonged to no
 * identity in particular. It now carries the mark, and the name is set in the
 * app's own ink.
 */
const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(({ isCollapsed }, ref) => {
  return (
    <Dialog aria-describedby={undefined}>
      <DialogTrigger asChild>
        <button
          ref={ref}
          aria-label="About gcrdings"
          className={`mb-2 flex cursor-pointer items-center gap-2 rounded-lg border-none bg-transparent p-0 transition-opacity hover:opacity-80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
            isCollapsed ? "justify-center" : "w-full justify-start px-1"
          }`}
        >
          <BrandMark size={isCollapsed ? 32 : 26} />
          {!isCollapsed && (
            <span className="text-[15px] font-semibold tracking-tight text-foreground">
              gcrdings
            </span>
          )}
        </button>
      </DialogTrigger>
      <DialogContent>
        <VisuallyHidden>
          <DialogTitle>About gcrdings</DialogTitle>
        </VisuallyHidden>
        <About />
      </DialogContent>
    </Dialog>
  );
});

Logo.displayName = "Logo";

export default Logo;
