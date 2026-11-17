import { useLayoutEffect, useRef } from "react";
import { Button } from "./Button";
import { Icon } from "./Icon";
import { Input } from "./Input";
import { ScrollArea } from "./ScrollArea";
import { Surface } from "./Surface";
import { useSelect, type SelectProps } from "./use-select";
import { useSelectPosition } from "./use-select-position";

export function SelectMenu({ props, model }: { props: SelectProps; model: ReturnType<typeof useSelect> }) {
  const list = useRef<HTMLDivElement>(null), search = useRef<HTMLInputElement>(null);
  const position = useSelectPosition(model.trigger, model.menu, list, model.dismiss);
  const placed = position !== null;
  useLayoutEffect(() => { if (placed && props.searchLabel) search.current?.focus(); }, [placed, props.searchLabel]);
  useLayoutEffect(() => {
    if (position && model.activeId) document.getElementById(model.activeId)?.scrollIntoView({ block: "nearest" });
  }, [model.activeId, position?.maxHeight]);
  return <div className="ccc-select-menu" ref={model.menu} data-side={position?.side}
    style={position ? { left: position.left, top: position.top, width: position.width, maxHeight: position.maxHeight }
      : { visibility: "hidden" }} onKeyDown={model.keyDown}>
    <Surface variant="card" className="ccc-select-panel">
      {props.searchLabel && <Input ref={search} type="search" aria-label={props.searchLabel} placeholder={props.searchLabel}
        aria-controls={model.listId} aria-activedescendant={model.activeId} value={model.query}
        onChange={event => { model.setQuery(event.target.value); model.setActive(null); }} />}
      {props.loading && <p className="ccc-select-message" role="status">读取中…</p>}
      {props.error && <div className="ccc-select-feedback"><p role="alert">{props.error}</p>
        {props.onRetry && <Button icon="refresh" variant="secondary" size="compact" disabled={props.loading} onClick={props.onRetry}>重试</Button>}</div>}
      <ScrollArea className="ccc-select-scroll" aria-label={`${props.label}列表滚动区`} tabIndex={-1}>
        <div ref={list} id={model.listId} role="listbox" aria-label={props.label}>
          {model.choices.map((option, index) => <Button key={option.value} id={`${model.listId}-${index}`} variant="ghost"
            role="option" tabIndex={-1} className="ccc-select-option" aria-selected={option.value === props.value}
            data-active={option === model.current} disabled={option.disabled} style={option.style}
            onPointerMove={() => { if (!option.disabled) model.setActive(option.value); }}
            onPointerDown={event => event.preventDefault()} onClick={() => model.choose(option.value)}>
            <span>{option.label}</span>{option.description && <small>{option.description}</small>}
            {option.value === props.value && <Icon name="check-circle" />}
          </Button>)}
          {!model.choices.length && <p className="ccc-select-message" role="status">没有可选项</p>}
        </div>
      </ScrollArea>
    </Surface>
  </div>;
}
