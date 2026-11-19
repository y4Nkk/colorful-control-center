import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { Button } from "./Button";
import { Icon } from "./Icon";
import { Input } from "./Input";
import { useSelect, type SelectProps } from "./use-select";
import { useSelectPosition } from "./use-select-position";

export function SelectMenu({ props, model }: { props: SelectProps; model: ReturnType<typeof useSelect> }) {
  const list = useRef<HTMLDivElement>(null), search = useRef<HTMLInputElement>(null);
  const [scrollCues, setScrollCues] = useState({ up: false, down: false });
  const position = useSelectPosition(model.trigger, model.menu, list, model.dismiss);
  const placed = position !== null;
  const measureScroll = () => {
    const viewport = list.current;
    if (!viewport) return;
    const { scrollTop, clientHeight, scrollHeight } = viewport;
    const up = scrollTop > 1;
    const down = Math.ceil(scrollTop + clientHeight) < scrollHeight;
    setScrollCues(current => current.up === up && current.down === down ? current : { up, down });
  };
  useLayoutEffect(() => { if (placed && props.searchLabel) search.current?.focus({ preventScroll: true }); }, [placed, props.searchLabel]);
  useLayoutEffect(() => {
    if (placed && model.activeId) document.getElementById(model.activeId)?.scrollIntoView({ block: "nearest" });
  }, [placed, model.activeId, position?.maxHeight]);
  useLayoutEffect(measureScroll, [model.choices, position?.maxHeight, scrollCues]);
  return <div className="ccc-select-menu" ref={model.menu} data-side={position?.side}
    style={position ? { left: position.left, top: position.top, maxHeight: position.maxHeight }
      : { visibility: "hidden" }} onKeyDown={model.keyDown}>
    {(props.searchLabel || props.loading || props.error) && <div className="ccc-select-header">
      {props.searchLabel && <Input ref={search} type="search" aria-label={props.searchLabel} placeholder={props.searchLabel}
        aria-controls={model.listId} aria-activedescendant={model.activeId} value={model.query}
        onChange={event => { model.setQuery(event.target.value); model.setActive(null); }} />}
      {props.loading && <p className="ccc-select-message" role="status">读取中…</p>}
      {props.error && <div className="ccc-select-feedback"><p role="alert">{props.error}</p>
        {props.onRetry && <Button icon="refresh" variant="secondary" size="compact" disabled={props.loading} onClick={props.onRetry}>重试</Button>}</div>}
    </div>}
    {placed && scrollCues.up && <SelectScrollCue direction="up" list={list} />}
    <div className="ccc-select-viewport" ref={list} id={model.listId} role="listbox" aria-label={props.label} tabIndex={-1} onScroll={measureScroll}>
      {model.choices.map((option, index) => <Button key={option.value} id={`${model.listId}-${index}`} variant="field"
        role="option" tabIndex={-1} className="ccc-select-option" aria-selected={option.value === props.value}
        data-active={option === model.current} disabled={option.disabled} style={option.style}
        onPointerMove={() => { if (!option.disabled) model.setActive(option.value); }}
        onPointerDown={event => event.preventDefault()} onClick={() => model.choose(option.value)}>
        <span>{option.label}</span>{option.description && <small>{option.description}</small>}
        {option.value === props.value && <Icon name="check" size={16} className="ccc-select-check" />}
      </Button>)}
      {!model.choices.length && <p className="ccc-select-message" role="status">没有可选项</p>}
    </div>
    {placed && scrollCues.down && <SelectScrollCue direction="down" list={list} />}
  </div>;
}

function SelectScrollCue({ direction, list }: { direction: "up" | "down"; list: RefObject<HTMLDivElement | null> }) {
  const timer = useRef<number | undefined>(undefined);
  const stop = () => window.clearInterval(timer.current);
  const scroll = () => {
    const viewport = list.current;
    if (!viewport) return;
    const row = viewport.querySelector<HTMLElement>('[role="option"][data-active="true"]') ?? viewport.firstElementChild;
    const step = row instanceof HTMLElement ? row.offsetHeight : viewport.clientHeight;
    viewport.scrollTop += (direction === "up" ? -1 : 1) * step;
  };
  useEffect(() => () => window.clearInterval(timer.current), []);
  return <Button variant="field" size="compact" className="ccc-select-scroll-cue" tabIndex={-1} aria-hidden="true"
    data-direction={direction} onPointerLeave={stop} onPointerCancel={stop}
    onPointerEnter={event => {
      if (event.pointerType === "touch") return;
      stop();
      timer.current = window.setInterval(scroll, 50);
    }}
    onPointerDown={event => { event.preventDefault(); scroll(); }}>
    <Icon name={direction === "up" ? "chevron-up" : "chevron-down"} size={16} />
  </Button>;
}
