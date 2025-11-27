import React, { cloneElement, useMemo, useRef, useState } from 'react';
import {
  useFloating,
  offset,
  flip,
  shift,
  useHover,
  useFocus,
  useDismiss,
  useRole,
  FloatingPortal,
  arrow,
  useInteractions,
  autoUpdate,
  useClientPoint,
} from '@floating-ui/react';

const Tooltip = ({ children, content, placement: initialPlacement = 'bottom', delay = 200 }) => {
  const arrowRef = useRef(null);
  const [open, setOpen] = useState(false);

  const middleware = useMemo(
    () => [
      offset({ mainAxis: 10, crossAxis: 0 }),
      flip({ fallbackAxisSideDirection: 'end' }),
      shift({ padding: 6 }),
      arrow({ element: arrowRef }),
    ],
    []
  );

  const { refs, floatingStyles, context, middlewareData, placement } = useFloating({
    placement: initialPlacement,
    open,
    onOpenChange: setOpen,
    middleware,
    whileElementsMounted: autoUpdate,
  });

  const hover = useHover(context, { move: true, delay: { open: delay, close: 40 } });
  const focus = useFocus(context);
  const dismiss = useDismiss(context, { escapeKey: true });
  const role = useRole(context, { role: 'tooltip' });
  const clientPoint = useClientPoint(context, { axis: 'both' });

  const { getReferenceProps, getFloatingProps } = useInteractions([hover, focus, dismiss, role, clientPoint]);

  const reference = cloneElement(children, {
    ...getReferenceProps(children.props),
    ref: refs.setReference,
  });

  return (
    <>
      {reference}
      {open && (
        <FloatingPortal>
          <div
            className="valknut-tooltip"
            ref={refs.setFloating}
            style={floatingStyles}
            data-placement={placement}
            {...getFloatingProps()}
          >
            {typeof content === 'function' ? content() : content}
            <span
              ref={arrowRef}
              className="valknut-tooltip-arrow"
              style={{
                left: middlewareData.arrow?.x != null ? `${middlewareData.arrow.x}px` : '',
                top: middlewareData.arrow?.y != null ? `${middlewareData.arrow.y}px` : '',
              }}
            />
          </div>
        </FloatingPortal>
      )}
    </>
  );
};

export default Tooltip;
