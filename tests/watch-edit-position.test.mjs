import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import * as ts from "typescript";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pageSource = fs.readFileSync(path.join(repoRoot, "src", "routes", "+page.svelte"), "utf8");
const scriptSource = /<script\b[^>]*>([\s\S]*?)<\/script>/.exec(pageSource)?.[1];

assert.ok(scriptSource, "+page.svelte must contain its TypeScript script");

const sourceFile = ts.createSourceFile(
  "+page.svelte.ts",
  scriptSource,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TS,
);

assert.deepEqual(
  sourceFile.parseDiagnostics.map((diagnostic) => diagnostic.messageText),
  [],
  "the source guard must parse the complete page script before inspecting functions",
);

const functions = new Map();

function visitFunctions(node) {
  if (ts.isFunctionDeclaration(node) && node.name && node.body) {
    functions.set(node.name.text, node);
  }
  ts.forEachChild(node, visitFunctions);
}

visitFunctions(sourceFile);

function functionNamed(name) {
  const node = functions.get(name);
  assert.ok(node, `+page.svelte must define ${name}`);
  return node;
}

function descendants(node, predicate) {
  const found = [];
  function visit(child) {
    if (predicate(child)) found.push(child);
    ts.forEachChild(child, visit);
  }
  visit(node);
  return found;
}

function callsNamed(node, name) {
  return descendants(
    node,
    (child) =>
      ts.isCallExpression(child) && ts.isIdentifier(child.expression) && child.expression.text === name,
  );
}

function assignmentsTo(node, name) {
  return descendants(
    node,
    (child) =>
      ts.isBinaryExpression(child) &&
      child.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
      ts.isIdentifier(child.left) &&
      child.left.text === name,
  );
}

function propertyAssignments(node, objectName, propertyName) {
  return descendants(
    node,
    (child) =>
      ts.isBinaryExpression(child) &&
      child.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
      ts.isPropertyAccessExpression(child.left) &&
      ts.isIdentifier(child.left.expression) &&
      child.left.expression.text === objectName &&
      child.left.name.text === propertyName,
  );
}

function compact(node) {
  return node.getText(sourceFile).replaceAll(/\s+/g, "");
}

function callArguments(call) {
  return call.arguments.map((argument) => compact(argument));
}

function assertSingleCall(node, name) {
  const calls = callsNamed(node, name);
  assert.equal(calls.length, 1, `${name} must be called exactly once in ${node.name?.text ?? "the guarded block"}`);
  return calls[0];
}

test("successful manual watch edits enter the dedicated preserved-position path", () => {
  const setWatched = functionNamed("setWatched");
  const capture = assertSingleCall(setWatched, "watchEditBrowseOrigin");
  const refresh = assertSingleCall(setWatched, "refreshAfterWatchEdit");
  const serverEdit = descendants(
    setWatched,
    (node) =>
      ts.isCallExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "invoke" &&
      ts.isStringLiteral(node.arguments[0]) &&
      node.arguments[0].text === "set_watched",
  );

  assert.equal(serverEdit.length, 1, "setWatched must make one set_watched request");
  assert.deepEqual(
    callArguments(serverEdit[0]),
    ['"set_watched"', "{item,played}"],
    "the watched command must receive the complete immutable title/backing identity",
  );
  assert.ok(
    capture.getStart(sourceFile) < serverEdit[0].getStart(sourceFile),
    "the originating browse root must be captured before the server request awaits",
  );
  assert.deepEqual(callArguments(refresh), ["browseOrigin"]);
  assert.ok(
    serverEdit[0].getStart(sourceFile) < refresh.getStart(sourceFile),
    "only a confirmed server edit may start browse revalidation",
  );
  const playedAssignment = propertyAssignments(setWatched, "item", "played");
  const offsetAssignment = propertyAssignments(setWatched, "item", "viewOffsetMs");
  assert.deepEqual(playedAssignment.map(compact), ["item.played=played"]);
  assert.deepEqual(offsetAssignment.map(compact), ["item.viewOffsetMs=0"]);
  assert.ok(
    serverEdit[0].getStart(sourceFile) < playedAssignment[0].getStart(sourceFile) &&
      playedAssignment[0].getStart(sourceFile) < refresh.getStart(sourceFile),
    "the confirmed local badge must publish before server-authoritative revalidation",
  );
  const partial = descendants(
    setWatched,
    (node) =>
      ts.isIfStatement(node) && compact(node.expression) === "result.failedSources>0",
  );
  assert.equal(partial.length, 1, "partial multi-source success must have one warning path");
  const partialStatus = assertSingleCall(partial[0], "publishEditStatus");
  assert.equal(
    compact(partialStatus.arguments.at(-1)),
    "false",
    "a partial source failure must publish a non-destructive action-owned warning",
  );
  assert.equal(callsNamed(setWatched, "resetAndLoad").length, 0, "manual edits must not reset the grid");
  assert.equal(
    callsNamed(setWatched, "refreshWatchState").length,
    0,
    "manual edits must not re-enter the general browse refresh",
  );

  const dispatcher = functionNamed("refreshAfterWatchEdit");
  assert.equal(callsNamed(dispatcher, "resetAndLoad").length, 0, "the edit dispatcher must never reset browse");
  assertSingleCall(dispatcher, "reloadBrowseAfterWatchEdit");
  assertSingleCall(dispatcher, "rerunQueryAfterWatchEdit");

  const listingReturn = descendants(dispatcher, ts.isReturnStatement).find((statement) =>
    ts.isConditionalExpression(statement.expression),
  );
  assert.ok(listingReturn, "the edit dispatcher must select a refresh by captured root kind");
  const branch = listingReturn.expression;
  assert.equal(compact(branch.condition), 'origin.kind==="listing"');
  assert.equal(compact(branch.whenTrue), "reloadBrowseAfterWatchEdit(origin)");
  assert.equal(compact(branch.whenFalse), "rerunQueryAfterWatchEdit(origin)");
});

test("listing identity and page fetching are shared by pagination and buffered reloads", () => {
  const current = compact(functionNamed("currentListingRequest"));
  for (const field of [
    "ratingKey:here.ratingKey",
    "sectionType:activeType",
    "sourceId:activeSource",
    "sectionKey:active.key",
    "sectionType:active.sectionType",
    "binding:active.binding??0",
    "sort",
  ]) {
    assert.ok(current.includes(field), `the immutable listing descriptor must retain ${field}`);
  }

  const same = compact(functionNamed("sameListingRequest"));
  for (const equality of [
    "a.ratingKey===b.ratingKey",
    "a.sectionType===b.sectionType",
    "a.sourceId===b.sourceId",
    "a.sectionKey===b.sectionKey",
    "a.binding===b.binding",
    "a.sort===b.sort",
  ]) {
    assert.ok(same.includes(equality), `listing ownership must compare ${equality}`);
  }

  const loadMore = functionNamed("loadMore");
  assertSingleCall(loadMore, "currentListingRequest");
  const fetch = assertSingleCall(loadMore, "fetchListingPage");
  assert.deepEqual(callArguments(fetch), ["request", "offset", "PAGE"]);
});

test("manual-edit buffering refills from zero and publishes the complete snapshot once", () => {
  const reload = functionNamed("reloadBrowseAfterWatchEdit");
  const reloadText = compact(reload);

  assert.ok(
    reloadText.includes("consttargetDepth=Math.max(offset,items.length)"),
    "buffering must preserve the depth already loaded",
  );
  assert.ok(reloadText.includes("constbuffered:Item[]=[]"), "the replacement must be built off-screen");
  assert.ok(reloadText.includes("letstart=0"), "authoritative revalidation must restart at offset zero");
  assert.ok(
    reloadText.includes("while(buffered.length<targetDepth&&refreshedHasMore)"),
    "buffering must refill every page needed to reach the prior depth",
  );

  const fetch = assertSingleCall(reload, "fetchListingPage");
  assert.deepEqual(callArguments(fetch), ["origin.request", "start", "PAGE"]);

  const loop = descendants(reload, ts.isWhileStatement)[0];
  assert.ok(loop, "buffering must use a bounded refill loop");
  assert.equal(assignmentsTo(loop, "items").length, 0, "a partial page must never publish to the grid");
  assert.equal(assignmentsTo(loop, "offset").length, 0, "a partial page must never publish pagination");
  assert.equal(assignmentsTo(loop, "hasMore").length, 0, "a partial page must never publish pagination");
  assert.ok(compact(loop).includes("buffered.push(...page)"), "each fetched page must append only to the buffer");
  assert.ok(compact(loop).includes("start+=page.length"), "the refill must advance by the received page");
  assert.ok(compact(loop).includes("refreshedHasMore=page.length>=PAGE"), "a short page must end refill");

  const itemAssignments = assignmentsTo(reload, "items");
  const offsetAssignments = assignmentsTo(reload, "offset");
  const hasMoreAssignments = assignmentsTo(reload, "hasMore");
  assert.deepEqual(itemAssignments.map(compact), ["items=buffered"], "the mounted grid must publish once");
  assert.deepEqual(offsetAssignments.map(compact), ["offset=buffered.length"]);
  assert.deepEqual(hasMoreAssignments.map(compact), ["hasMore=refreshedHasMore"]);

  assert.doesNotMatch(
    reloadText,
    /items=(?:\[\]|newArray\(\))|items\.length=0|items\.splice\(0/,
    "manual-edit buffering must never blank the mounted grid",
  );
  assert.doesNotMatch(reloadText, /loading=true/, "manual-edit buffering must not enable the grid skeleton");
});

test("buffer publication, scroll restoration, and guard release retain exact ownership", () => {
  const owns = compact(functionNamed("ownsWatchEditOrigin"));
  assert.ok(owns.includes('mode!=="browse"||navEpoch!==origin.epoch'), "navigation must supersede old edits");
  assert.ok(
    owns.includes("sameListingRequest(origin.request,currentListingRequest())"),
    "a same-looking but differently bound listing must not accept stale work",
  );

  const reload = functionNamed("reloadBrowseAfterWatchEdit");
  const fetch = assertSingleCall(reload, "fetchListingPage");
  const publish = assignmentsTo(reload, "items")[0];
  const ticks = callsNamed(reload, "tick");
  const restore = assertSingleCall(reload, "restoreGridScroll");
  assert.equal(ticks.length, 1, "scroll restoration must wait for one DOM update");
  assert.ok(ts.isAwaitExpression(ticks[0].parent), "tick must be awaited before measuring the rebuilt grid");

  const ownerReturns = descendants(
    reload,
    (node) =>
      ts.isIfStatement(node) &&
      compact(node.expression).includes("myGen!==loadGen") &&
      compact(node.expression).includes("!ownsWatchEditOrigin(origin)") &&
      descendants(node.thenStatement, ts.isReturnStatement).length > 0,
  );
  assert.equal(
    ownerReturns.length,
    3,
    "ownership must be checked after each fetch, before publication, and after the DOM update",
  );
  assert.ok(fetch.getStart(sourceFile) < ownerReturns[0].getStart(sourceFile));
  assert.ok(ownerReturns[0].getStart(sourceFile) < publish.getStart(sourceFile));
  assert.ok(ownerReturns[1].getStart(sourceFile) < publish.getStart(sourceFile));
  assert.ok(publish.getStart(sourceFile) < ticks[0].getStart(sourceFile));
  assert.ok(ticks[0].getStart(sourceFile) < ownerReturns[2].getStart(sourceFile));
  assert.ok(ownerReturns[2].getStart(sourceFile) < restore.getStart(sourceFile));
  assert.deepEqual(callArguments(restore), ["savedScrollTop"]);

  const tryStatement = descendants(reload, ts.isTryStatement)[0];
  assert.ok(tryStatement?.finallyBlock, "the pagination guard must always settle");
  assert.ok(
    compact(tryStatement.finallyBlock).includes("if(myGen===loadGen)loadingMore=false"),
    "only the current load generation may release loadingMore",
  );
});

test("one-shot search and person reruns restore scroll only for the originating root", () => {
  const rerun = functionNamed("rerunQueryAfterWatchEdit");
  const text = compact(rerun);
  assert.ok(text.includes("constsavedScrollTop=gridEl?.scrollTop??0"));
  assertSingleCall(rerun, "runSearch");
  assertSingleCall(rerun, "runPersonView");
  assert.ok(
    text.includes("constmyGen=loadGen"),
    "the rerun must retain the generation synchronously claimed by the query",
  );
  const tickCall = assertSingleCall(rerun, "tick");
  const restore = assertSingleCall(rerun, "restoreGridScroll");
  assert.ok(ts.isAwaitExpression(tickCall.parent), "query scroll restoration must wait for the DOM");
  assert.ok(
    tickCall.getStart(sourceFile) < restore.getStart(sourceFile),
    "query scroll restoration must happen after the awaited DOM update",
  );
  assert.deepEqual(callArguments(restore), ["savedScrollTop"]);
  assert.ok(
    descendants(
      rerun,
      (node) =>
        ts.isIfStatement(node) &&
        compact(node.expression).includes("myGen!==loadGen") &&
        compact(node.expression).includes("!ownsWatchEditOrigin(origin)"),
    ).length >= 2,
    "query publication and scroll restoration must both yield to newer root ownership",
  );
});
