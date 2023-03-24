//! Composable layouts.

mod align;
mod columns;
mod container;
#[path = "enum.rs"]
mod enum_;
mod flow;
mod fragment;
mod grid;
mod hide;
mod list;
mod measure;
mod pad;
mod page;
mod par;
mod place;
mod regions;
mod repeat;
mod spacing;
mod stack;
mod table;
mod terms;
mod transform;

use std::mem;

use typed_arena::Arena;
use typst::diag::SourceResult;
use typst::eval::{Scope, Tracer};
use typst::model::{applicable, realize, StyleVecBuilder};

pub use self::align::AlignElem;
pub use self::columns::{ColbreakElem, ColumnsElem};
pub use self::container::{BlockElem, BoxElem, Sizing};
pub use self::enum_::{EnumElem, EnumItem};
pub use self::flow::FlowElem;
pub use self::fragment::Fragment;
use self::grid::GridLayouter;
pub use self::grid::{GridElem, TrackSizings};
pub use self::hide::HideElem;
pub use self::list::{ListElem, ListItem};
pub use self::measure::measure;
pub use self::pad::PadElem;
pub use self::page::{PageElem, PagebreakElem};
pub use self::par::{ParElem, ParbreakElem, SpanMapper};
pub use self::place::PlaceElem;
pub use self::regions::Regions;
pub use self::repeat::RepeatElem;
pub use self::spacing::{HElem, Spacing, VElem};
pub use self::stack::StackElem;
pub use self::table::TableElem;
pub use self::terms::{TermItem, TermsElem};
pub use self::transform::{MoveElem, RotateElem, ScaleElem};
use crate::math::{EquationElem, LayoutMath};
use crate::meta::DocumentElem;
use crate::prelude::*;
use crate::shared::BehavedBuilder;
use crate::text::{LinebreakElem, SmartQuoteElem, SpaceElem, TextElem};
use crate::visualize::{CircleElem, EllipseElem, ImageElem, RectElem, SquareElem};

pub(super) fn define(scope: &mut Scope) {
    scope.define("page", PageElem::func());
    scope.define("pagebreak", PagebreakElem::func());
    scope.define("v", VElem::func());
    scope.define("par", ParElem::func());
    scope.define("parbreak", ParbreakElem::func());
    scope.define("h", HElem::func());
    scope.define("box", BoxElem::func());
    scope.define("block", BlockElem::func());
    scope.define("list", ListElem::func());
    scope.define("enum", EnumElem::func());
    scope.define("terms", TermsElem::func());
    scope.define("table", TableElem::func());
    scope.define("stack", StackElem::func());
    scope.define("grid", GridElem::func());
    scope.define("columns", ColumnsElem::func());
    scope.define("colbreak", ColbreakElem::func());
    scope.define("place", PlaceElem::func());
    scope.define("align", AlignElem::func());
    scope.define("pad", PadElem::func());
    scope.define("repeat", RepeatElem::func());
    scope.define("move", MoveElem::func());
    scope.define("scale", ScaleElem::func());
    scope.define("rotate", RotateElem::func());
    scope.define("hide", HideElem::func());
    scope.define("measure", measure);
}

/// Root-level layout.
pub trait LayoutRoot {
    /// Layout into one frame per page.
    ///
    /// # Errors
    ///
    /// Propagates errors from layouting children.
    fn layout_root(
        &self,
        vt: &mut Vt<'_>,
        styles: StyleChain<'_>,
    ) -> SourceResult<Document>;
}

impl LayoutRoot for Content {
    fn layout_root(
        &self,
        vt: &mut Vt<'_>,
        styles: StyleChain<'_>,
    ) -> SourceResult<Document> {
        #[comemo::memoize]
        fn cached(
            content: &Content,
            world: Tracked<'_, dyn World>,
            tracer: TrackedMut<'_, Tracer>,
            provider: TrackedMut<'_, StabilityProvider>,
            introspector: Tracked<'_, Introspector>,
            styles: StyleChain<'_>,
        ) -> SourceResult<Document> {
            let mut vt = Vt { world, tracer, provider, introspector };
            let scratch = Scratch::default();
            let (realized, styles) = realize_root(&mut vt, &scratch, content, styles)?;
            realized
                .with::<dyn LayoutRoot>()
                .unwrap()
                .layout_root(&mut vt, styles)
        }

        cached(
            self,
            vt.world,
            TrackedMut::reborrow_mut(&mut vt.tracer),
            TrackedMut::reborrow_mut(&mut vt.provider),
            vt.introspector,
            styles,
        )
    }
}

/// Layout into regions.
pub trait Layout {
    /// Layout into one frame per region.
    ///
    /// # Errors
    ///
    /// Propagates errors from layouting children.
    fn layout(
        &self,
        vt: &mut Vt<'_>,
        styles: StyleChain<'_>,
        regions: Regions<'_>,
    ) -> SourceResult<Fragment>;

    /// Layout without side effects.
    ///
    /// This element must be layouted again in the same order for the results to be valid.
    ///
    /// # Errors
    ///
    /// Propagates errors from layouting children.
    fn measure(
        &self,
        vt: &mut Vt<'_>,
        styles: StyleChain<'_>,
        regions: Regions<'_>,
    ) -> SourceResult<Fragment> {
        vt.provider.save();
        let result = self.layout(vt, styles, regions);
        vt.provider.restore();
        result
    }
}

impl Layout for Content {
    fn layout(
        &self,
        vt: &mut Vt<'_>,
        styles: StyleChain<'_>,
        regions: Regions<'_>,
    ) -> SourceResult<Fragment> {
        #[comemo::memoize]
        fn cached(
            content: &Content,
            world: Tracked<'_, dyn World>,
            tracer: TrackedMut<'_, Tracer>,
            provider: TrackedMut<'_, StabilityProvider>,
            introspector: Tracked<'_, Introspector>,
            styles: StyleChain<'_>,
            regions: Regions<'_>,
        ) -> SourceResult<Fragment> {
            let mut vt = Vt { world, tracer, provider, introspector };
            let scratch = Scratch::default();
            let (realized, styles) = realize_block(&mut vt, &scratch, content, styles)?;
            realized
                .with::<dyn Layout>()
                .unwrap()
                .layout(&mut vt, styles, regions)
        }

        cached(
            self,
            vt.world,
            TrackedMut::reborrow_mut(&mut vt.tracer),
            TrackedMut::reborrow_mut(&mut vt.provider),
            vt.introspector,
            styles,
            regions,
        )
    }
}

/// Realize into an element that is capable of root-level layout.
fn realize_root<'a>(
    vt: &mut Vt<'_>,
    scratch: &'a Scratch<'a>,
    content: &'a Content,
    styles: StyleChain<'a>,
) -> SourceResult<(Content, StyleChain<'a>)> {
    if content.can::<dyn LayoutRoot>() && !applicable(content, styles) {
        return Ok((content.clone(), styles));
    }

    let mut builder = Builder::new(vt, scratch, true);
    builder.accept(content, styles)?;
    builder.interrupt_page(Some(styles))?;
    let (pages, shared) = builder.doc.unwrap().pages.finish();
    Ok((DocumentElem::new(pages.to_vec()).pack(), shared))
}

/// Realize into an element that is capable of block-level layout.
fn realize_block<'a>(
    vt: &mut Vt<'_>,
    scratch: &'a Scratch<'a>,
    content: &'a Content,
    styles: StyleChain<'a>,
) -> SourceResult<(Content, StyleChain<'a>)> {
    if content.can::<dyn Layout>()
        && !content.is::<RectElem>()
        && !content.is::<SquareElem>()
        && !content.is::<EllipseElem>()
        && !content.is::<CircleElem>()
        && !content.is::<ImageElem>()
        && !applicable(content, styles)
    {
        return Ok((content.clone(), styles));
    }

    let mut builder = Builder::new(vt, scratch, false);
    builder.accept(content, styles)?;
    builder.interrupt_par()?;
    let (children, shared) = builder.flow.0.finish();
    Ok((FlowElem::new(children.to_vec()).pack(), shared))
}

/// Builds a document or a flow element from content.
struct Builder<'a, 'v, 't> {
    /// The virtual typesetter.
    vt: &'v mut Vt<'t>,
    /// Scratch arenas for building.
    scratch: &'a Scratch<'a>,
    /// The current document building state.
    doc: Option<DocBuilder<'a>>,
    /// The current flow building state.
    flow: FlowBuilder<'a>,
    /// The current paragraph building state.
    par: ParBuilder<'a>,
    /// The current list building state.
    list: ListBuilder<'a>,
}

/// Temporary storage arenas for building.
#[derive(Default)]
struct Scratch<'a> {
    /// An arena where intermediate style chains are stored.
    styles: Arena<StyleChain<'a>>,
    /// An arena where intermediate content resulting from show rules is stored.
    content: Arena<Content>,
}

impl<'a, 'v, 't> Builder<'a, 'v, 't> {
    fn new(vt: &'v mut Vt<'t>, scratch: &'a Scratch<'a>, top: bool) -> Self {
        Self {
            vt,
            scratch,
            doc: top.then(DocBuilder::default),
            flow: FlowBuilder::default(),
            par: ParBuilder::default(),
            list: ListBuilder::default(),
        }
    }

    fn accept(
        &mut self,
        mut content: &'a Content,
        styles: StyleChain<'a>,
    ) -> SourceResult<()> {
        if content.can::<dyn LayoutMath>() && !content.is::<EquationElem>() {
            content =
                self.scratch.content.alloc(EquationElem::new(content.clone()).pack());
        }

        if let Some((elem, local)) = content.to_styled() {
            return self.styled(elem, local, styles);
        }

        if let Some(children) = content.to_sequence() {
            for elem in children {
                self.accept(elem, styles)?;
            }
            return Ok(());
        }

        if let Some(realized) = realize(self.vt, content, styles)? {
            let stored = self.scratch.content.alloc(realized);
            return self.accept(stored, styles);
        }

        if self.list.accept(content, styles) {
            return Ok(());
        }

        self.interrupt_list()?;

        if self.list.accept(content, styles) {
            return Ok(());
        }

        if self.par.accept(content, styles) {
            return Ok(());
        }

        self.interrupt_par()?;

        if self.flow.accept(content, styles) {
            return Ok(());
        }

        let keep = content
            .to::<PagebreakElem>()
            .map_or(false, |pagebreak| !pagebreak.weak(styles));

        self.interrupt_page(keep.then_some(styles))?;

        if let Some(doc) = &mut self.doc {
            if doc.accept(content, styles) {
                return Ok(());
            }
        }

        #[allow(clippy::redundant_else /* clarity */)]
        if content.is::<PagebreakElem>() {
            bail!(content.span(), "pagebreaks are not allowed inside of containers");
        } else {
            bail!(content.span(), "{} is not allowed here", content.func().name());
        }
    }

    fn styled(
        &mut self,
        elem: &'a Content,
        map: &'a Styles,
        styles: StyleChain<'a>,
    ) -> SourceResult<()> {
        let stored = self.scratch.styles.alloc(styles);
        let styles = stored.chain(map);
        self.interrupt_style(map, None)?;
        self.accept(elem, styles)?;
        self.interrupt_style(map, Some(styles))?;
        Ok(())
    }

    fn interrupt_style(
        &mut self,
        local: &Styles,
        outer: Option<StyleChain<'a>>,
    ) -> SourceResult<()> {
        if let Some(Some(span)) = local.interruption::<DocumentElem>() {
            if self.doc.is_none() {
                bail!(span, "document set rules are not allowed inside of containers");
            }
            if outer.is_none()
                && (!self.flow.0.is_empty()
                    || !self.par.0.is_empty()
                    || !self.list.items.is_empty())
            {
                bail!(span, "document set rules must appear before any content");
            }
        } else if let Some(Some(span)) = local.interruption::<PageElem>() {
            if self.doc.is_none() {
                bail!(span, "page configuration is not allowed inside of containers");
            }
            self.interrupt_page(outer)?;
        } else if local.interruption::<ParElem>().is_some()
            || local.interruption::<AlignElem>().is_some()
        {
            self.interrupt_par()?;
        } else if local.interruption::<ListElem>().is_some()
            || local.interruption::<EnumElem>().is_some()
            || local.interruption::<TermsElem>().is_some()
        {
            self.interrupt_list()?;
        }
        Ok(())
    }

    fn interrupt_list(&mut self) -> SourceResult<()> {
        if !self.list.items.is_empty() {
            let staged = mem::take(&mut self.list.staged);
            let (list, styles) = mem::take(&mut self.list).finish();
            let stored = self.scratch.content.alloc(list);
            self.accept(stored, styles)?;
            for (content, styles) in staged {
                self.accept(content, styles)?;
            }
        }
        Ok(())
    }

    fn interrupt_par(&mut self) -> SourceResult<()> {
        self.interrupt_list()?;
        if !self.par.0.is_empty() {
            let (par, styles) = mem::take(&mut self.par).finish();
            let stored = self.scratch.content.alloc(par);
            self.accept(stored, styles)?;
        }

        Ok(())
    }

    fn interrupt_page(&mut self, styles: Option<StyleChain<'a>>) -> SourceResult<()> {
        self.interrupt_par()?;
        let Some(doc) = &mut self.doc else { return Ok(()) };
        if !self.flow.0.is_empty() || (doc.keep_next && styles.is_some()) {
            let (flow, shared) = mem::take(&mut self.flow).0.finish();
            let styles = if shared == StyleChain::default() {
                styles.unwrap_or_default()
            } else {
                shared
            };
            let page = PageElem::new(FlowElem::new(flow.to_vec()).pack()).pack();
            let stored = self.scratch.content.alloc(page);
            self.accept(stored, styles)?;
        }
        Ok(())
    }
}

/// Accepts pagebreaks and pages.
struct DocBuilder<'a> {
    /// The page runs built so far.
    pages: StyleVecBuilder<'a, Content>,
    /// Whether to keep a following page even if it is empty.
    keep_next: bool,
}

impl<'a> DocBuilder<'a> {
    fn accept(&mut self, content: &Content, styles: StyleChain<'a>) -> bool {
        if let Some(pagebreak) = content.to::<PagebreakElem>() {
            self.keep_next = !pagebreak.weak(styles);
            return true;
        }

        if content.is::<PageElem>() {
            self.pages.push(content.clone(), styles);
            self.keep_next = false;
            return true;
        }

        false
    }
}

impl Default for DocBuilder<'_> {
    fn default() -> Self {
        Self { pages: StyleVecBuilder::new(), keep_next: true }
    }
}

/// Accepts flow content.
#[derive(Default)]
struct FlowBuilder<'a>(BehavedBuilder<'a>, bool);

impl<'a> FlowBuilder<'a> {
    fn accept(&mut self, content: &'a Content, styles: StyleChain<'a>) -> bool {
        if content.is::<ParbreakElem>() {
            self.1 = true;
            return true;
        }

        let last_was_parbreak = self.1;
        self.1 = false;

        if content.is::<VElem>()
            || content.is::<ColbreakElem>()
            || content.is::<MetaElem>()
        {
            self.0.push(content.clone(), styles);
            return true;
        }

        if content.can::<dyn Layout>() || content.is::<ParElem>() {
            let is_tight_list = if let Some(elem) = content.to::<ListElem>() {
                elem.tight(styles)
            } else if let Some(elem) = content.to::<EnumElem>() {
                elem.tight(styles)
            } else if let Some(elem) = content.to::<TermsElem>() {
                elem.tight(styles)
            } else {
                false
            };

            if !last_was_parbreak && is_tight_list {
                let leading = ParElem::leading_in(styles);
                let spacing = VElem::list_attach(leading.into());
                self.0.push(spacing.pack(), styles);
            }

            let above = BlockElem::above_in(styles);
            let below = BlockElem::below_in(styles);
            self.0.push(above.pack(), styles);
            self.0.push(content.clone(), styles);
            self.0.push(below.pack(), styles);
            return true;
        }

        false
    }
}

/// Accepts paragraph content.
#[derive(Default)]
struct ParBuilder<'a>(BehavedBuilder<'a>);

impl<'a> ParBuilder<'a> {
    fn accept(&mut self, content: &'a Content, styles: StyleChain<'a>) -> bool {
        if content.is::<MetaElem>() {
            if !self.0.is_basically_empty() {
                self.0.push(content.clone(), styles);
                return true;
            }
        } else if content.is::<SpaceElem>()
            || content.is::<TextElem>()
            || content.is::<HElem>()
            || content.is::<LinebreakElem>()
            || content.is::<SmartQuoteElem>()
            || content.to::<EquationElem>().map_or(false, |elem| !elem.block(styles))
            || content.is::<BoxElem>()
        {
            self.0.push(content.clone(), styles);
            return true;
        }

        false
    }

    fn finish(self) -> (Content, StyleChain<'a>) {
        let (children, shared) = self.0.finish();
        (ParElem::new(children.to_vec()).pack(), shared)
    }
}

/// Accepts list / enum items, spaces, paragraph breaks.
struct ListBuilder<'a> {
    /// The list items collected so far.
    items: StyleVecBuilder<'a, Content>,
    /// Whether the list contains no paragraph breaks.
    tight: bool,
    /// Trailing content for which it is unclear whether it is part of the list.
    staged: Vec<(&'a Content, StyleChain<'a>)>,
}

impl<'a> ListBuilder<'a> {
    fn accept(&mut self, content: &'a Content, styles: StyleChain<'a>) -> bool {
        if !self.items.is_empty()
            && (content.is::<SpaceElem>() || content.is::<ParbreakElem>())
        {
            self.staged.push((content, styles));
            return true;
        }

        if (content.is::<ListItem>()
            || content.is::<EnumItem>()
            || content.is::<TermItem>())
            && self
                .items
                .elems()
                .next()
                .map_or(true, |first| first.func() == content.func())
        {
            self.items.push(content.clone(), styles);
            self.tight &= self.staged.drain(..).all(|(t, _)| !t.is::<ParbreakElem>());
            return true;
        }

        false
    }

    fn finish(self) -> (Content, StyleChain<'a>) {
        let (items, shared) = self.items.finish();
        let item = items.items().next().unwrap();
        let output = if item.is::<ListItem>() {
            ListElem::new(
                items
                    .iter()
                    .map(|(item, local)| {
                        let item = item.to::<ListItem>().unwrap();
                        item.clone().with_body(item.body().styled_with_map(local.clone()))
                    })
                    .collect::<Vec<_>>(),
            )
            .with_tight(self.tight)
            .pack()
        } else if item.is::<EnumItem>() {
            EnumElem::new(
                items
                    .iter()
                    .map(|(item, local)| {
                        let item = item.to::<EnumItem>().unwrap();
                        item.clone().with_body(item.body().styled_with_map(local.clone()))
                    })
                    .collect::<Vec<_>>(),
            )
            .with_tight(self.tight)
            .pack()
        } else if item.is::<TermItem>() {
            TermsElem::new(
                items
                    .iter()
                    .map(|(item, local)| {
                        let item = item.to::<TermItem>().unwrap();
                        item.clone()
                            .with_term(item.term().styled_with_map(local.clone()))
                            .with_description(
                                item.description().styled_with_map(local.clone()),
                            )
                    })
                    .collect::<Vec<_>>(),
            )
            .with_tight(self.tight)
            .pack()
        } else {
            unreachable!()
        };
        (output, shared)
    }
}

impl Default for ListBuilder<'_> {
    fn default() -> Self {
        Self {
            items: StyleVecBuilder::default(),
            tight: true,
            staged: vec![],
        }
    }
}
