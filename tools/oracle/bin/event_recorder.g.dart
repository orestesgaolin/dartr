// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_listener.py`.
//
// A parser listener that records every call as JSON, for oracle mode
// `events` (see events.dart). Generated from
// pkg/_fe_analyzer_shared/lib/src/parser/listener.dart.

// ignore_for_file: implementation_imports

import 'package:_fe_analyzer_shared/src/experiments/flags.dart';
import 'package:_fe_analyzer_shared/src/messages/codes.dart';
import 'package:_fe_analyzer_shared/src/parser/assert.dart';
import 'package:_fe_analyzer_shared/src/parser/block_kind.dart';
import 'package:_fe_analyzer_shared/src/parser/constructor_reference_context.dart';
import 'package:_fe_analyzer_shared/src/parser/declaration_kind.dart';
import 'package:_fe_analyzer_shared/src/parser/formal_parameter_kind.dart';
import 'package:_fe_analyzer_shared/src/parser/identifier_context.dart';
import 'package:_fe_analyzer_shared/src/parser/listener.dart';
import 'package:_fe_analyzer_shared/src/parser/member_kind.dart';
import 'package:_fe_analyzer_shared/src/scanner/error_token.dart';
import 'package:_fe_analyzer_shared/src/scanner/token.dart';

class EventRecorder extends Listener {
  final List<Object?> events = [];
  final List<Object?> errors = [];

  Object? t(Token? token) =>
      token == null ? null : [token.offset, token.lexeme];

  Object? e(ErrorToken token) => [token.charOffset, token.errorCode.name];

  Object? m(Message message) => {
    'code': message.code.name,
    'msg': message.problemMessage,
    if (message.correctionMessage != null) 'fix': message.correctionMessage,
  };

  @override
  void beginArguments(Token token) {
    events.add(['beginArguments', t(token)]);
  }

  @override
  void endArguments(int count, Token beginToken, Token endToken) {
    events.add(['endArguments', count, t(beginToken), t(endToken)]);
  }

  @override
  void handleObjectPatternFields(int count, Token beginToken, Token endToken) {
    events.add(['handleObjectPatternFields', count, t(beginToken), t(endToken)]);
  }

  @override
  void handleAsyncModifier(Token? asyncToken, Token? starToken) {
    events.add(['handleAsyncModifier', t(asyncToken), t(starToken)]);
  }

  @override
  void beginAwaitExpression(Token token) {
    events.add(['beginAwaitExpression', t(token)]);
  }

  @override
  void endAwaitExpression(Token beginToken, Token endToken) {
    events.add(['endAwaitExpression', t(beginToken), t(endToken)]);
  }

  @override
  void endInvalidAwaitExpression(Token beginToken, Token endToken, MessageCode errorCode,) {
    events.add(['endInvalidAwaitExpression', t(beginToken), t(endToken), errorCode.name]);
  }

  @override
  void beginBlock(Token token, BlockKind blockKind) {
    events.add(['beginBlock', t(token), blockKind.name]);
  }

  @override
  void endBlock(int count, Token beginToken, Token endToken, BlockKind blockKind,) {
    events.add(['endBlock', count, t(beginToken), t(endToken), blockKind.name]);
  }

  @override
  void handleInvalidTopLevelBlock(Token token) {
    events.add(['handleInvalidTopLevelBlock', t(token)]);
  }

  @override
  void beginCascade(Token token) {
    events.add(['beginCascade', t(token)]);
  }

  @override
  void endCascade() {
    events.add(['endCascade']);
  }

  @override
  void beginCaseExpression(Token caseKeyword) {
    events.add(['beginCaseExpression', t(caseKeyword)]);
  }

  @override
  void endCaseExpression(Token caseKeyword, Token? when, Token colon) {
    events.add(['endCaseExpression', t(caseKeyword), t(when), t(colon)]);
  }

  @override
  void beginClassOrMixinOrExtensionBody(DeclarationKind kind, Token token) {
    events.add(['beginClassOrMixinOrExtensionBody', kind.name, t(token)]);
  }

  @override
  void endClassOrMixinOrExtensionBody(DeclarationKind kind, int memberCount, Token beginToken, Token endToken,) {
    events.add(['endClassOrMixinOrExtensionBody', kind.name, memberCount, t(beginToken), t(endToken)]);
  }

  @override
  void beginClassOrMixinOrNamedMixinApplicationPrelude(Token token) {
    events.add(['beginClassOrMixinOrNamedMixinApplicationPrelude', t(token)]);
  }

  @override
  void beginClassDeclaration(Token begin, Token? abstractToken, Token? sealedToken, Token? baseToken, Token? interfaceToken, Token? finalToken, Token? augmentToken, Token? mixinToken, Token name,) {
    events.add(['beginClassDeclaration', t(begin), t(abstractToken), t(sealedToken), t(baseToken), t(interfaceToken), t(finalToken), t(augmentToken), t(mixinToken), t(name)]);
  }

  @override
  void handleClassExtends(Token? extendsKeyword, int typeCount) {
    events.add(['handleClassExtends', t(extendsKeyword), typeCount]);
  }

  @override
  void handleImplements(Token? implementsKeyword, int interfacesCount) {
    events.add(['handleImplements', t(implementsKeyword), interfacesCount]);
  }

  @override
  void handleClassHeader(Token begin, Token classKeyword, Token? nativeToken) {
    events.add(['handleClassHeader', t(begin), t(classKeyword), t(nativeToken)]);
  }

  @override
  void handleRecoverDeclarationHeader(DeclarationHeaderKind kind) {
    events.add(['handleRecoverDeclarationHeader', kind.name]);
  }

  @override
  void endClassDeclaration(Token beginToken, Token endToken) {
    events.add(['endClassDeclaration', t(beginToken), t(endToken)]);
  }

  @override
  void handleNoClassBody(Token semicolonToken) {
    events.add(['handleNoClassBody', t(semicolonToken)]);
  }

  @override
  void handleNoExtensionTypeBody(Token semicolonToken) {
    events.add(['handleNoExtensionTypeBody', t(semicolonToken)]);
  }

  @override
  void beginMixinDeclaration(Token beginToken, Token? augmentToken, Token? baseToken, Token mixinKeyword, Token name,) {
    events.add(['beginMixinDeclaration', t(beginToken), t(augmentToken), t(baseToken), t(mixinKeyword), t(name)]);
  }

  @override
  void handleMixinOn(Token? onKeyword, int typeCount) {
    events.add(['handleMixinOn', t(onKeyword), typeCount]);
  }

  @override
  void handleMixinHeader(Token mixinKeyword) {
    events.add(['handleMixinHeader', t(mixinKeyword)]);
  }

  @override
  void handleRecoverMixinHeader() {
    events.add(['handleRecoverMixinHeader']);
  }

  @override
  void handleNoMixinBody(Token semicolonToken) {
    events.add(['handleNoMixinBody', t(semicolonToken)]);
  }

  @override
  void endMixinDeclaration(Token beginToken, Token endToken) {
    events.add(['endMixinDeclaration', t(beginToken), t(endToken)]);
  }

  @override
  void beginUncategorizedTopLevelDeclaration(Token token) {
    events.add(['beginUncategorizedTopLevelDeclaration', t(token)]);
  }

  @override
  void beginExtensionDeclarationPrelude(Token extensionKeyword) {
    events.add(['beginExtensionDeclarationPrelude', t(extensionKeyword)]);
  }

  @override
  void beginExtensionDeclaration(Token? augmentToken, Token extensionKeyword, Token? name,) {
    events.add(['beginExtensionDeclaration', t(augmentToken), t(extensionKeyword), t(name)]);
  }

  @override
  void endExtensionDeclaration(Token beginToken, Token extensionKeyword, Token? onKeyword, Token endToken,) {
    events.add(['endExtensionDeclaration', t(beginToken), t(extensionKeyword), t(onKeyword), t(endToken)]);
  }

  @override
  void handleNoExtensionBody(Token semicolonToken) {
    events.add(['handleNoExtensionBody', t(semicolonToken)]);
  }

  @override
  void beginExtensionTypeDeclaration(Token? augmentKeyword, Token extensionKeyword, Token name,) {
    events.add(['beginExtensionTypeDeclaration', t(augmentKeyword), t(extensionKeyword), t(name)]);
  }

  @override
  void endExtensionTypeDeclaration(Token beginToken, Token? augmentToken, Token extensionKeyword, Token typeKeyword, Token endToken,) {
    events.add(['endExtensionTypeDeclaration', t(beginToken), t(augmentToken), t(extensionKeyword), t(typeKeyword), t(endToken)]);
  }

  @override
  void beginPrimaryConstructor(Token beginToken) {
    events.add(['beginPrimaryConstructor', t(beginToken)]);
  }

  @override
  void endPrimaryConstructor(DeclarationKind kind, Token beginToken, Token endToken, Token? constKeyword, bool hasConstructorName,) {
    events.add(['endPrimaryConstructor', kind.name, t(beginToken), t(endToken), t(constKeyword), hasConstructorName]);
  }

  @override
  void handleNoPrimaryConstructor(DeclarationKind kind, Token token, Token? constKeyword,) {
    events.add(['handleNoPrimaryConstructor', kind.name, t(token), t(constKeyword)]);
  }

  @override
  void beginPrimaryConstructorBody(Token token) {
    events.add(['beginPrimaryConstructorBody', t(token)]);
  }

  @override
  void endPrimaryConstructorBody(Token beginToken, Token? beginInitializers, Token endToken,) {
    events.add(['endPrimaryConstructorBody', t(beginToken), t(beginInitializers), t(endToken)]);
  }

  @override
  void beginCombinators(Token token) {
    events.add(['beginCombinators', t(token)]);
  }

  @override
  void endCombinators(int count) {
    events.add(['endCombinators', count]);
  }

  @override
  void beginCompilationUnit(Token token) {
    events.add(['beginCompilationUnit', t(token)]);
  }

  @override
  void handleDirectivesOnly() {
    events.add(['handleDirectivesOnly']);
  }

  @override
  void endCompilationUnit(int count, Token token) {
    events.add(['endCompilationUnit', count, t(token)]);
  }

  @override
  void beginConstLiteral(Token token) {
    events.add(['beginConstLiteral', t(token)]);
  }

  @override
  void endConstLiteral(Token endToken) {
    events.add(['endConstLiteral', t(endToken)]);
  }

  @override
  void beginConstructorReference(Token start) {
    events.add(['beginConstructorReference', t(start)]);
  }

  @override
  void endConstructorReference(Token start, Token? periodBeforeName, Token endToken, ConstructorReferenceContext constructorReferenceContext,) {
    events.add(['endConstructorReference', t(start), t(periodBeforeName), t(endToken), constructorReferenceContext.name]);
  }

  @override
  void beginDoWhileStatement(Token token) {
    events.add(['beginDoWhileStatement', t(token)]);
  }

  @override
  void endDoWhileStatement(Token doKeyword, Token whileKeyword, Token endToken,) {
    events.add(['endDoWhileStatement', t(doKeyword), t(whileKeyword), t(endToken)]);
  }

  @override
  void beginDoWhileStatementBody(Token token) {
    events.add(['beginDoWhileStatementBody', t(token)]);
  }

  @override
  void endDoWhileStatementBody(Token token) {
    events.add(['endDoWhileStatementBody', t(token)]);
  }

  @override
  void beginWhileStatementBody(Token token) {
    events.add(['beginWhileStatementBody', t(token)]);
  }

  @override
  void endWhileStatementBody(Token endToken) {
    events.add(['endWhileStatementBody', t(endToken)]);
  }

  @override
  void beginEnumDeclarationPrelude(Token enumKeyword) {
    events.add(['beginEnumDeclarationPrelude', t(enumKeyword)]);
  }

  @override
  void beginEnumDeclaration(Token beginToken, Token? augmentToken, Token enumKeyword, Token name,) {
    events.add(['beginEnumDeclaration', t(beginToken), t(augmentToken), t(enumKeyword), t(name)]);
  }

  @override
  void endEnumDeclaration(Token beginToken, Token enumKeyword, Token leftBrace, int memberCount, Token endToken,) {
    events.add(['endEnumDeclaration', t(beginToken), t(enumKeyword), t(leftBrace), memberCount, t(endToken)]);
  }

  @override
  void handleEnumElements(Token elementsEndToken, int elementsCount) {
    events.add(['handleEnumElements', t(elementsEndToken), elementsCount]);
  }

  @override
  void handleEnumHeader(Token? augmentToken, Token enumKeyword, Token leftBrace,) {
    events.add(['handleEnumHeader', t(augmentToken), t(enumKeyword), t(leftBrace)]);
  }

  @override
  void beginEnumBody(Token token) {
    events.add(['beginEnumBody', t(token)]);
  }

  @override
  void endEnumBody(Token beginToken, Token endToken) {
    events.add(['endEnumBody', t(beginToken), t(endToken)]);
  }

  @override
  void handleNoEnumBody(Token semicolonToken) {
    events.add(['handleNoEnumBody', t(semicolonToken)]);
  }

  @override
  void handleEnumElement(Token beginToken, Token? augmentToken) {
    events.add(['handleEnumElement', t(beginToken), t(augmentToken)]);
  }

  @override
  void beginExport(Token token) {
    events.add(['beginExport', t(token)]);
  }

  @override
  void endExport(Token exportKeyword, Token semicolon) {
    events.add(['endExport', t(exportKeyword), t(semicolon)]);
  }

  @override
  void handleExtraneousExpression(Token token, Message message) {
    events.add(['handleExtraneousExpression', t(token), m(message)]);
  }

  @override
  void handleExpressionStatement(Token beginToken, Token endToken) {
    events.add(['handleExpressionStatement', t(beginToken), t(endToken)]);
  }

  @override
  void beginFactory(DeclarationKind declarationKind, Token lastConsumed, Token? augmentToken, Token? externalToken, Token? constToken,) {
    events.add(['beginFactory', declarationKind.name, t(lastConsumed), t(augmentToken), t(externalToken), t(constToken)]);
  }

  @override
  void endFactory(DeclarationKind kind, Token beginToken, Token factoryKeyword, Token endToken,) {
    events.add(['endFactory', kind.name, t(beginToken), t(factoryKeyword), t(endToken)]);
  }

  @override
  void beginFormalParameter(Token token, MemberKind kind, Token? requiredToken, Token? covariantToken, Token? varFinalOrConst,) {
    events.add(['beginFormalParameter', t(token), kind.name, t(requiredToken), t(covariantToken), t(varFinalOrConst)]);
  }

  @override
  void endFormalParameter(Token? varOrFinal, Token? thisKeyword, Token? superKeyword, Token? periodAfterThisOrSuper, Token nameToken, Token? initializerStart, Token? initializerEnd, FormalParameterKind kind, MemberKind memberKind,) {
    events.add(['endFormalParameter', t(varOrFinal), t(thisKeyword), t(superKeyword), t(periodAfterThisOrSuper), t(nameToken), t(initializerStart), t(initializerEnd), kind.name, memberKind.name]);
  }

  @override
  void handleNoFormalParameters(Token token, MemberKind kind) {
    events.add(['handleNoFormalParameters', t(token), kind.name]);
  }

  @override
  void beginFormalParameters(Token token, MemberKind kind) {
    events.add(['beginFormalParameters', t(token), kind.name]);
  }

  @override
  void endFormalParameters(int count, Token beginToken, Token endToken, MemberKind kind,) {
    events.add(['endFormalParameters', count, t(beginToken), t(endToken), kind.name]);
  }

  @override
  void endFields(DeclarationKind kind, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, int count, Token beginToken, Token endToken,) {
    events.add(['endFields', kind.name, t(abstractToken), t(augmentToken), t(externalToken), t(staticToken), t(covariantToken), t(lateToken), t(varFinalOrConst), count, t(beginToken), t(endToken)]);
  }

  @override
  void handleForInitializerEmptyStatement(Token token) {
    events.add(['handleForInitializerEmptyStatement', t(token)]);
  }

  @override
  void handleForInitializerExpressionStatement(Token token, bool forIn) {
    events.add(['handleForInitializerExpressionStatement', t(token), forIn]);
  }

  @override
  void handleForInitializerLocalVariableDeclaration(Token token, bool forIn) {
    events.add(['handleForInitializerLocalVariableDeclaration', t(token), forIn]);
  }

  @override
  void handleForInitializerPatternVariableAssignment(Token keyword, Token equals,) {
    events.add(['handleForInitializerPatternVariableAssignment', t(keyword), t(equals)]);
  }

  @override
  void beginForStatement(Token token) {
    events.add(['beginForStatement', t(token)]);
  }

  @override
  void handleForLoopParts(Token forKeyword, Token leftParen, Token leftSeparator, Token rightSeparator, int updateExpressionCount,) {
    events.add(['handleForLoopParts', t(forKeyword), t(leftParen), t(leftSeparator), t(rightSeparator), updateExpressionCount]);
  }

  @override
  void endForStatement(Token endToken) {
    events.add(['endForStatement', t(endToken)]);
  }

  @override
  void beginForStatementBody(Token token) {
    events.add(['beginForStatementBody', t(token)]);
  }

  @override
  void endForStatementBody(Token endToken) {
    events.add(['endForStatementBody', t(endToken)]);
  }

  @override
  void handleForInLoopParts(Token? awaitToken, Token forToken, Token leftParenthesis, Token? patternKeyword, Token inKeyword,) {
    events.add(['handleForInLoopParts', t(awaitToken), t(forToken), t(leftParenthesis), t(patternKeyword), t(inKeyword)]);
  }

  @override
  void endForIn(Token endToken) {
    events.add(['endForIn', t(endToken)]);
  }

  @override
  void beginForInExpression(Token token) {
    events.add(['beginForInExpression', t(token)]);
  }

  @override
  void endForInExpression(Token token) {
    events.add(['endForInExpression', t(token)]);
  }

  @override
  void beginForInBody(Token token) {
    events.add(['beginForInBody', t(token)]);
  }

  @override
  void endForInBody(Token endToken) {
    events.add(['endForInBody', t(endToken)]);
  }

  @override
  void beginNamedFunctionExpression(Token token) {
    events.add(['beginNamedFunctionExpression', t(token)]);
  }

  @override
  void endNamedFunctionExpression(Token endToken) {
    events.add(['endNamedFunctionExpression', t(endToken)]);
  }

  @override
  void beginLocalFunctionDeclaration(Token token) {
    events.add(['beginLocalFunctionDeclaration', t(token)]);
  }

  @override
  void endLocalFunctionDeclaration(Token endToken) {
    events.add(['endLocalFunctionDeclaration', t(endToken)]);
  }

  @override
  void beginBlockFunctionBody(Token token) {
    events.add(['beginBlockFunctionBody', t(token)]);
  }

  @override
  void endBlockFunctionBody(int count, Token beginToken, Token endToken) {
    events.add(['endBlockFunctionBody', count, t(beginToken), t(endToken)]);
  }

  @override
  void handleNoFunctionBody(Token token) {
    events.add(['handleNoFunctionBody', t(token)]);
  }

  @override
  void handleFunctionBodySkipped(Token beginToken, Token endToken, bool isExpressionBody,) {
    events.add(['handleFunctionBodySkipped', t(beginToken), t(endToken), isExpressionBody]);
  }

  @override
  void beginFunctionName(Token token) {
    events.add(['beginFunctionName', t(token)]);
  }

  @override
  void endFunctionName(Token beginToken, Token token, bool isFunctionExpression,) {
    events.add(['endFunctionName', t(beginToken), t(token), isFunctionExpression]);
  }

  @override
  void beginTypedef(Token token) {
    events.add(['beginTypedef', t(token)]);
  }

  @override
  void endTypedef(Token? augmentToken, Token typedefKeyword, Token? equals, Token endToken,) {
    events.add(['endTypedef', t(augmentToken), t(typedefKeyword), t(equals), t(endToken)]);
  }

  @override
  void handleClassWithClause(Token withKeyword) {
    events.add(['handleClassWithClause', t(withKeyword)]);
  }

  @override
  void handleClassNoWithClause() {
    events.add(['handleClassNoWithClause']);
  }

  @override
  void handleEnumWithClause(Token withKeyword) {
    events.add(['handleEnumWithClause', t(withKeyword)]);
  }

  @override
  void handleEnumNoWithClause() {
    events.add(['handleEnumNoWithClause']);
  }

  @override
  void handleMixinWithClause(Token withKeyword) {
    events.add(['handleMixinWithClause', t(withKeyword)]);
  }

  @override
  void beginNamedMixinApplication(Token beginToken, Token? abstractToken, Token? sealedToken, Token? baseToken, Token? interfaceToken, Token? finalToken, Token? augmentToken, Token? mixinToken, Token name,) {
    events.add(['beginNamedMixinApplication', t(beginToken), t(abstractToken), t(sealedToken), t(baseToken), t(interfaceToken), t(finalToken), t(augmentToken), t(mixinToken), t(name)]);
  }

  @override
  void handleNamedMixinApplicationWithClause(Token withKeyword) {
    events.add(['handleNamedMixinApplicationWithClause', t(withKeyword)]);
  }

  @override
  void endNamedMixinApplication(Token begin, Token classKeyword, Token equals, Token? implementsKeyword, Token endToken,) {
    events.add(['endNamedMixinApplication', t(begin), t(classKeyword), t(equals), t(implementsKeyword), t(endToken)]);
  }

  @override
  void beginHide(Token hideKeyword) {
    events.add(['beginHide', t(hideKeyword)]);
  }

  @override
  void endHide(Token hideKeyword) {
    events.add(['endHide', t(hideKeyword)]);
  }

  @override
  void handleIdentifierList(int count) {
    events.add(['handleIdentifierList', count]);
  }

  @override
  void beginTypeList(Token token) {
    events.add(['beginTypeList', t(token)]);
  }

  @override
  void endTypeList(int count) {
    events.add(['endTypeList', count]);
  }

  @override
  void beginIfStatement(Token token) {
    events.add(['beginIfStatement', t(token)]);
  }

  @override
  void endIfStatement(Token ifToken, Token? elseToken, Token endToken) {
    events.add(['endIfStatement', t(ifToken), t(elseToken), t(endToken)]);
  }

  @override
  void beginThenStatement(Token token) {
    events.add(['beginThenStatement', t(token)]);
  }

  @override
  void endThenStatement(Token beginToken, Token endToken) {
    events.add(['endThenStatement', t(beginToken), t(endToken)]);
  }

  @override
  void beginElseStatement(Token token) {
    events.add(['beginElseStatement', t(token)]);
  }

  @override
  void endElseStatement(Token beginToken, Token endToken) {
    events.add(['endElseStatement', t(beginToken), t(endToken)]);
  }

  @override
  void beginImport(Token importKeyword) {
    events.add(['beginImport', t(importKeyword)]);
  }

  @override
  void handleImportPrefix(Token? deferredKeyword, Token? asKeyword) {
    events.add(['handleImportPrefix', t(deferredKeyword), t(asKeyword)]);
  }

  @override
  void endImport(Token importKeyword, Token? semicolon) {
    events.add(['endImport', t(importKeyword), t(semicolon)]);
  }

  @override
  void handleRecoverImport(Token? semicolon) {
    events.add(['handleRecoverImport', t(semicolon)]);
  }

  @override
  void beginConditionalUris(Token token) {
    events.add(['beginConditionalUris', t(token)]);
  }

  @override
  void endConditionalUris(int count) {
    events.add(['endConditionalUris', count]);
  }

  @override
  void beginConditionalUri(Token ifKeyword) {
    events.add(['beginConditionalUri', t(ifKeyword)]);
  }

  @override
  void endConditionalUri(Token ifKeyword, Token leftParen, Token? equalSign) {
    events.add(['endConditionalUri', t(ifKeyword), t(leftParen), t(equalSign)]);
  }

  @override
  void handleDottedName(int count, Token firstIdentifier) {
    events.add(['handleDottedName', count, t(firstIdentifier)]);
  }

  @override
  void beginImplicitCreationExpression(Token token) {
    events.add(['beginImplicitCreationExpression', t(token)]);
  }

  @override
  void endImplicitCreationExpression(Token token, Token openAngleBracket) {
    events.add(['endImplicitCreationExpression', t(token), t(openAngleBracket)]);
  }

  @override
  void beginInitializedIdentifier(Token token) {
    events.add(['beginInitializedIdentifier', t(token)]);
  }

  @override
  void endInitializedIdentifier(Token nameToken) {
    events.add(['endInitializedIdentifier', t(nameToken)]);
  }

  @override
  void beginFieldInitializer(Token token) {
    events.add(['beginFieldInitializer', t(token)]);
  }

  @override
  void endFieldInitializer(Token assignment, Token endToken) {
    events.add(['endFieldInitializer', t(assignment), t(endToken)]);
  }

  @override
  void handleNoFieldInitializer(Token token) {
    events.add(['handleNoFieldInitializer', t(token)]);
  }

  @override
  void beginVariableInitializer(Token token) {
    events.add(['beginVariableInitializer', t(token)]);
  }

  @override
  void endVariableInitializer(Token assignmentOperator) {
    events.add(['endVariableInitializer', t(assignmentOperator)]);
  }

  @override
  void handleNoVariableInitializer(Token token) {
    events.add(['handleNoVariableInitializer', t(token)]);
  }

  @override
  void beginInitializer(Token token) {
    events.add(['beginInitializer', t(token)]);
  }

  @override
  void endInitializer(Token endToken) {
    events.add(['endInitializer', t(endToken)]);
  }

  @override
  void beginInitializers(Token token) {
    events.add(['beginInitializers', t(token)]);
  }

  @override
  void endInitializers(int count, Token beginToken, Token endToken) {
    events.add(['endInitializers', count, t(beginToken), t(endToken)]);
  }

  @override
  void handleNoInitializers() {
    events.add(['handleNoInitializers']);
  }

  @override
  void handleInvalidExpression(Token token) {
    events.add(['handleInvalidExpression', t(token)]);
  }

  @override
  void handleInvalidFunctionBody(Token token) {
    events.add(['handleInvalidFunctionBody', t(token)]);
  }

  @override
  void handleInvalidTypeReference(Token token) {
    events.add(['handleInvalidTypeReference', t(token)]);
  }

  @override
  void handleLabel(Token token) {
    events.add(['handleLabel', t(token)]);
  }

  @override
  void beginLabeledStatement(Token token, int labelCount) {
    events.add(['beginLabeledStatement', t(token), labelCount]);
  }

  @override
  void endLabeledStatement(int labelCount) {
    events.add(['endLabeledStatement', labelCount]);
  }

  @override
  void beginLibraryAugmentation(Token augmentKeyword, Token libraryKeyword) {
    events.add(['beginLibraryAugmentation', t(augmentKeyword), t(libraryKeyword)]);
  }

  @override
  void endLibraryAugmentation(Token augmentKeyword, Token libraryKeyword, Token semicolon,) {
    events.add(['endLibraryAugmentation', t(augmentKeyword), t(libraryKeyword), t(semicolon)]);
  }

  @override
  void beginLibraryName(Token token) {
    events.add(['beginLibraryName', t(token)]);
  }

  @override
  void endLibraryName(Token libraryKeyword, Token semicolon, bool hasName) {
    events.add(['endLibraryName', t(libraryKeyword), t(semicolon), hasName]);
  }

  @override
  void handleLiteralMapEntry(Token colon, Token endToken, { Token? nullAwareKeyToken, Token? nullAwareValueToken, }) {
    events.add(['handleLiteralMapEntry', t(colon), t(endToken), t(nullAwareKeyToken), t(nullAwareValueToken)]);
  }

  @override
  void handleMapPatternEntry(Token colon, Token endToken) {
    events.add(['handleMapPatternEntry', t(colon), t(endToken)]);
  }

  @override
  void beginLiteralString(Token token) {
    events.add(['beginLiteralString', t(token)]);
  }

  @override
  void handleInterpolationExpression(Token leftBracket, Token? rightBracket) {
    events.add(['handleInterpolationExpression', t(leftBracket), t(rightBracket)]);
  }

  @override
  void endLiteralString(int interpolationCount, Token endToken) {
    events.add(['endLiteralString', interpolationCount, t(endToken)]);
  }

  @override
  void handleAdjacentStringLiterals(Token startToken, int literalCount) {
    events.add(['handleAdjacentStringLiterals', t(startToken), literalCount]);
  }

  @override
  void beginMember() {
    events.add(['beginMember']);
  }

  @override
  void handleInvalidMember(Token endToken) {
    events.add(['handleInvalidMember', t(endToken)]);
  }

  @override
  void endMember() {
    events.add(['endMember']);
  }

  @override
  void beginMethod(DeclarationKind declarationKind, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? varFinalOrConst, Token? getOrSet, Token name, String? enclosingDeclarationName,) {
    events.add(['beginMethod', declarationKind.name, t(augmentToken), t(externalToken), t(staticToken), t(covariantToken), t(varFinalOrConst), t(getOrSet), t(name), enclosingDeclarationName]);
  }

  @override
  void endMethod(DeclarationKind kind, Token? getOrSet, Token beginToken, Token beginParam, Token? beginInitializers, Token endToken,) {
    events.add(['endMethod', kind.name, t(getOrSet), t(beginToken), t(beginParam), t(beginInitializers), t(endToken)]);
  }

  @override
  void beginConstructor(DeclarationKind declarationKind, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? varFinalOrConst, Token? getOrSet, Token? newToken, Token name, String? enclosingDeclarationName,) {
    events.add(['beginConstructor', declarationKind.name, t(augmentToken), t(externalToken), t(staticToken), t(covariantToken), t(varFinalOrConst), t(getOrSet), t(newToken), t(name), enclosingDeclarationName]);
  }

  @override
  void endConstructor(DeclarationKind kind, Token beginToken, Token? newToken, Token beginParam, Token? beginInitializers, Token endToken,) {
    events.add(['endConstructor', kind.name, t(beginToken), t(newToken), t(beginParam), t(beginInitializers), t(endToken)]);
  }

  @override
  void beginMetadataStar(Token token) {
    events.add(['beginMetadataStar', t(token)]);
  }

  @override
  void endMetadataStar(int count) {
    events.add(['endMetadataStar', count]);
  }

  @override
  void beginMetadata(Token token) {
    events.add(['beginMetadata', t(token)]);
  }

  @override
  void endMetadata(Token beginToken, Token? periodBeforeName, Token endToken) {
    events.add(['endMetadata', t(beginToken), t(periodBeforeName), t(endToken)]);
  }

  @override
  void beginOptionalFormalParameters(Token token) {
    events.add(['beginOptionalFormalParameters', t(token)]);
  }

  @override
  void endOptionalFormalParameters(int count, Token beginToken, Token endToken, MemberKind kind,) {
    events.add(['endOptionalFormalParameters', count, t(beginToken), t(endToken), kind.name]);
  }

  @override
  void beginPart(Token token) {
    events.add(['beginPart', t(token)]);
  }

  @override
  void endPart(Token partKeyword, Token semicolon) {
    events.add(['endPart', t(partKeyword), t(semicolon)]);
  }

  @override
  void beginPartOf(Token token) {
    events.add(['beginPartOf', t(token)]);
  }

  @override
  void endPartOf(Token partKeyword, Token ofKeyword, Token semicolon, bool hasName,) {
    events.add(['endPartOf', t(partKeyword), t(ofKeyword), t(semicolon), hasName]);
  }

  @override
  void beginRedirectingFactoryBody(Token token) {
    events.add(['beginRedirectingFactoryBody', t(token)]);
  }

  @override
  void endRedirectingFactoryBody(Token beginToken, Token endToken) {
    events.add(['endRedirectingFactoryBody', t(beginToken), t(endToken)]);
  }

  @override
  void beginReturnStatement(Token token) {
    events.add(['beginReturnStatement', t(token)]);
  }

  @override
  void handleNativeFunctionBody(Token nativeToken, Token semicolon) {
    events.add(['handleNativeFunctionBody', t(nativeToken), t(semicolon)]);
  }

  @override
  void handleNativeFunctionBodyIgnored(Token nativeToken, Token semicolon) {
    events.add(['handleNativeFunctionBodyIgnored', t(nativeToken), t(semicolon)]);
  }

  @override
  void handleNativeFunctionBodySkipped(Token nativeToken, Token semicolon) {
    events.add(['handleNativeFunctionBodySkipped', t(nativeToken), t(semicolon)]);
  }

  @override
  void handleEmptyFunctionBody(Token semicolon) {
    events.add(['handleEmptyFunctionBody', t(semicolon)]);
  }

  @override
  void handleExpressionFunctionBody(Token arrowToken, Token? endToken) {
    events.add(['handleExpressionFunctionBody', t(arrowToken), t(endToken)]);
  }

  @override
  void endReturnStatement(bool hasExpression, Token beginToken, Token endToken,) {
    events.add(['endReturnStatement', hasExpression, t(beginToken), t(endToken)]);
  }

  @override
  void handleSend(Token beginToken, Token endToken) {
    events.add(['handleSend', t(beginToken), t(endToken)]);
  }

  @override
  void beginShow(Token showKeyword) {
    events.add(['beginShow', t(showKeyword)]);
  }

  @override
  void endShow(Token showKeyword) {
    events.add(['endShow', t(showKeyword)]);
  }

  @override
  void beginSwitchStatement(Token token) {
    events.add(['beginSwitchStatement', t(token)]);
  }

  @override
  void endSwitchStatement(Token switchKeyword, Token endToken) {
    events.add(['endSwitchStatement', t(switchKeyword), t(endToken)]);
  }

  @override
  void beginSwitchExpression(Token token) {
    events.add(['beginSwitchExpression', t(token)]);
  }

  @override
  void endSwitchExpression(Token switchKeyword, Token endToken) {
    events.add(['endSwitchExpression', t(switchKeyword), t(endToken)]);
  }

  @override
  void beginSwitchBlock(Token token) {
    events.add(['beginSwitchBlock', t(token)]);
  }

  @override
  void endSwitchBlock(int caseCount, Token beginToken, Token endToken) {
    events.add(['endSwitchBlock', caseCount, t(beginToken), t(endToken)]);
  }

  @override
  void beginSwitchExpressionBlock(Token token) {
    events.add(['beginSwitchExpressionBlock', t(token)]);
  }

  @override
  void endSwitchExpressionBlock(int caseCount, Token beginToken, Token endToken,) {
    events.add(['endSwitchExpressionBlock', caseCount, t(beginToken), t(endToken)]);
  }

  @override
  void beginLiteralSymbol(Token token) {
    events.add(['beginLiteralSymbol', t(token)]);
  }

  @override
  void endLiteralSymbol(Token hashToken, int identifierCount) {
    events.add(['endLiteralSymbol', t(hashToken), identifierCount]);
  }

  @override
  void handleThrowExpression(Token throwToken, Token endToken) {
    events.add(['handleThrowExpression', t(throwToken), t(endToken)]);
  }

  @override
  void beginRethrowStatement(Token token) {
    events.add(['beginRethrowStatement', t(token)]);
  }

  @override
  void endRethrowStatement(Token rethrowToken, Token endToken) {
    events.add(['endRethrowStatement', t(rethrowToken), t(endToken)]);
  }

  @override
  void endTopLevelDeclaration(Token endToken) {
    events.add(['endTopLevelDeclaration', t(endToken)]);
  }

  @override
  void handleInvalidTopLevelDeclaration(Token endToken) {
    events.add(['handleInvalidTopLevelDeclaration', t(endToken)]);
  }

  @override
  void beginTopLevelMember(Token token) {
    events.add(['beginTopLevelMember', t(token)]);
  }

  @override
  void beginFields(DeclarationKind declarationKind, Token? augmentToken, Token? abstractToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token lastConsumed,) {
    events.add(['beginFields', declarationKind.name, t(augmentToken), t(abstractToken), t(externalToken), t(staticToken), t(covariantToken), t(lateToken), t(varFinalOrConst), t(lastConsumed)]);
  }

  @override
  void endTopLevelFields(Token? augmentToken, Token? abstractToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, int count, Token beginToken, Token endToken,) {
    events.add(['endTopLevelFields', t(augmentToken), t(abstractToken), t(externalToken), t(staticToken), t(covariantToken), t(lateToken), t(varFinalOrConst), count, t(beginToken), t(endToken)]);
  }

  @override
  void beginTopLevelMethod(Token lastConsumed, Token? augmentToken, Token? externalToken,) {
    events.add(['beginTopLevelMethod', t(lastConsumed), t(augmentToken), t(externalToken)]);
  }

  @override
  void endTopLevelMethod(Token beginToken, Token? getOrSet, Token endToken) {
    events.add(['endTopLevelMethod', t(beginToken), t(getOrSet), t(endToken)]);
  }

  @override
  void beginTryStatement(Token token) {
    events.add(['beginTryStatement', t(token)]);
  }

  @override
  void beginCatchClause(Token token) {
    events.add(['beginCatchClause', t(token)]);
  }

  @override
  void endCatchClause(Token token) {
    events.add(['endCatchClause', t(token)]);
  }

  @override
  void handleCatchBlock(Token? onKeyword, Token? catchKeyword, Token? comma) {
    events.add(['handleCatchBlock', t(onKeyword), t(catchKeyword), t(comma)]);
  }

  @override
  void handleFinallyBlock(Token finallyKeyword) {
    events.add(['handleFinallyBlock', t(finallyKeyword)]);
  }

  @override
  void endTryStatement(int catchCount, Token tryKeyword, Token? finallyKeyword, Token endToken,) {
    events.add(['endTryStatement', catchCount, t(tryKeyword), t(finallyKeyword), t(endToken)]);
  }

  @override
  void handleType(Token beginToken, Token? questionMark) {
    events.add(['handleType', t(beginToken), t(questionMark)]);
  }

  @override
  void handleNonNullAssertExpression(Token bang) {
    events.add(['handleNonNullAssertExpression', t(bang)]);
  }

  @override
  void handleNullAssertPattern(Token bang) {
    events.add(['handleNullAssertPattern', t(bang)]);
  }

  @override
  void handleNullCheckPattern(Token question) {
    events.add(['handleNullCheckPattern', t(question)]);
  }

  @override
  void handleAssignedVariablePattern(Token variable) {
    events.add(['handleAssignedVariablePattern', t(variable)]);
  }

  @override
  void handleDeclaredVariablePattern(Token? keyword, Token variable, { required bool inAssignmentPattern, }) {
    events.add(['handleDeclaredVariablePattern', t(keyword), t(variable), inAssignmentPattern]);
  }

  @override
  void handleWildcardPattern(Token? keyword, Token wildcard) {
    events.add(['handleWildcardPattern', t(keyword), t(wildcard)]);
  }

  @override
  void handleNoName(Token token) {
    events.add(['handleNoName', t(token)]);
  }

  @override
  void beginRecordType(Token leftBracket) {
    events.add(['beginRecordType', t(leftBracket)]);
  }

  @override
  void endRecordType(Token leftBracket, Token? questionMark, int count, bool hasNamedFields,) {
    events.add(['endRecordType', t(leftBracket), t(questionMark), count, hasNamedFields]);
  }

  @override
  void beginRecordTypeEntry() {
    events.add(['beginRecordTypeEntry']);
  }

  @override
  void endRecordTypeEntry() {
    events.add(['endRecordTypeEntry']);
  }

  @override
  void beginRecordTypeNamedFields(Token leftBracket) {
    events.add(['beginRecordTypeNamedFields', t(leftBracket)]);
  }

  @override
  void endRecordTypeNamedFields(int count, Token leftBracket) {
    events.add(['endRecordTypeNamedFields', count, t(leftBracket)]);
  }

  @override
  void beginFunctionType(Token beginToken) {
    events.add(['beginFunctionType', t(beginToken)]);
  }

  @override
  void endFunctionType(Token functionToken, Token? questionMark) {
    events.add(['endFunctionType', t(functionToken), t(questionMark)]);
  }

  @override
  void beginTypeArguments(Token token) {
    events.add(['beginTypeArguments', t(token)]);
  }

  @override
  void endTypeArguments(int count, Token beginToken, Token endToken) {
    events.add(['endTypeArguments', count, t(beginToken), t(endToken)]);
  }

  @override
  void handleInvalidTypeArguments(Token token) {
    events.add(['handleInvalidTypeArguments', t(token)]);
  }

  @override
  void handleNoTypeArguments(Token token) {
    events.add(['handleNoTypeArguments', t(token)]);
  }

  @override
  void beginTypeVariable(Token token) {
    events.add(['beginTypeVariable', t(token)]);
  }

  @override
  void handleTypeVariablesDefined(Token token, int count) {
    events.add(['handleTypeVariablesDefined', t(token), count]);
  }

  @override
  void endTypeVariable(Token token, int index, Token? extendsOrSuper, Token? variance,) {
    events.add(['endTypeVariable', t(token), index, t(extendsOrSuper), t(variance)]);
  }

  @override
  void beginTypeVariables(Token token) {
    events.add(['beginTypeVariables', t(token)]);
  }

  @override
  void endTypeVariables(Token beginToken, Token endToken) {
    events.add(['endTypeVariables', t(beginToken), t(endToken)]);
  }

  @override
  void reportVarianceModifierNotEnabled(Token? variance) {
    events.add(['reportVarianceModifierNotEnabled', t(variance)]);
  }

  @override
  void beginFunctionExpression(Token token) {
    events.add(['beginFunctionExpression', t(token)]);
  }

  @override
  void endFunctionExpression(Token beginToken, Token endToken) {
    events.add(['endFunctionExpression', t(beginToken), t(endToken)]);
  }

  @override
  void beginVariablesDeclaration(Token token, Token? lateToken, Token? varFinalOrConst,) {
    events.add(['beginVariablesDeclaration', t(token), t(lateToken), t(varFinalOrConst)]);
  }

  @override
  void endVariablesDeclaration(int count, Token? endToken) {
    events.add(['endVariablesDeclaration', count, t(endToken)]);
  }

  @override
  void beginWhileStatement(Token token) {
    events.add(['beginWhileStatement', t(token)]);
  }

  @override
  void endWhileStatement(Token whileKeyword, Token endToken) {
    events.add(['endWhileStatement', t(whileKeyword), t(endToken)]);
  }

  @override
  void beginAsOperatorType(Token operator) {
    events.add(['beginAsOperatorType', t(operator)]);
  }

  @override
  void endAsOperatorType(Token operator) {
    events.add(['endAsOperatorType', t(operator)]);
  }

  @override
  void handleAsOperator(Token operator) {
    events.add(['handleAsOperator', t(operator)]);
  }

  @override
  void handleCastPattern(Token operator) {
    events.add(['handleCastPattern', t(operator)]);
  }

  @override
  void handleAssignmentExpression(Token token, Token endToken) {
    events.add(['handleAssignmentExpression', t(token), t(endToken)]);
  }

  @override
  void beginAnonymousMethodInvocation(Token token) {
    events.add(['beginAnonymousMethodInvocation', t(token)]);
  }

  @override
  void endAnonymousMethodInvocation(Token beginToken, Token? functionDefinition, Token endToken, { required bool isExpression, }) {
    events.add(['endAnonymousMethodInvocation', t(beginToken), t(functionDefinition), t(endToken), isExpression]);
  }

  @override
  void handleImplicitFormalParameters(Token token) {
    events.add(['handleImplicitFormalParameters', t(token)]);
  }

  @override
  void beginBinaryExpression(Token token) {
    events.add(['beginBinaryExpression', t(token)]);
  }

  @override
  void endBinaryExpression(Token token, Token endToken) {
    events.add(['endBinaryExpression', t(token), t(endToken)]);
  }

  @override
  void beginBinaryPattern(Token token) {
    events.add(['beginBinaryPattern', t(token)]);
  }

  @override
  void endBinaryPattern(Token operatorToken) {
    events.add(['endBinaryPattern', t(operatorToken)]);
  }

  @override
  void handleDotAccess(Token token, Token endToken, bool isNullAware) {
    events.add(['handleDotAccess', t(token), t(endToken), isNullAware]);
  }

  @override
  void handleCascadeAccess(Token token, Token endToken, bool isNullAware) {
    events.add(['handleCascadeAccess', t(token), t(endToken), isNullAware]);
  }

  @override
  void beginConditionalExpression(Token question) {
    events.add(['beginConditionalExpression', t(question)]);
  }

  @override
  void handleConditionalExpressionColon() {
    events.add(['handleConditionalExpressionColon']);
  }

  @override
  void endConditionalExpression(Token question, Token colon, Token endToken) {
    events.add(['endConditionalExpression', t(question), t(colon), t(endToken)]);
  }

  @override
  void beginConstExpression(Token constKeyword) {
    events.add(['beginConstExpression', t(constKeyword)]);
  }

  @override
  void endConstExpression(Token token) {
    events.add(['endConstExpression', t(token)]);
  }

  @override
  void handleConstFactory(Token constKeyword) {
    events.add(['handleConstFactory', t(constKeyword)]);
  }

  @override
  void beginForControlFlow(Token? awaitToken, Token forToken) {
    events.add(['beginForControlFlow', t(awaitToken), t(forToken)]);
  }

  @override
  void endForControlFlow(Token token) {
    events.add(['endForControlFlow', t(token)]);
  }

  @override
  void endForInControlFlow(Token token) {
    events.add(['endForInControlFlow', t(token)]);
  }

  @override
  void beginIfControlFlow(Token ifToken) {
    events.add(['beginIfControlFlow', t(ifToken)]);
  }

  @override
  void handleThenControlFlow(Token token) {
    events.add(['handleThenControlFlow', t(token)]);
  }

  @override
  void handleElseControlFlow(Token elseToken) {
    events.add(['handleElseControlFlow', t(elseToken)]);
  }

  @override
  void endIfControlFlow(Token token) {
    events.add(['endIfControlFlow', t(token)]);
  }

  @override
  void endIfElseControlFlow(Token token) {
    events.add(['endIfElseControlFlow', t(token)]);
  }

  @override
  void handleSpreadExpression(Token spreadToken) {
    events.add(['handleSpreadExpression', t(spreadToken)]);
  }

  @override
  void handleNullAwareElement(Token nullAwareToken) {
    events.add(['handleNullAwareElement', t(nullAwareToken)]);
  }

  @override
  void handleRestPattern(Token dots, {required bool hasSubPattern}) {
    events.add(['handleRestPattern', t(dots), hasSubPattern]);
  }

  @override
  void beginFunctionTypedFormalParameter(Token token) {
    events.add(['beginFunctionTypedFormalParameter', t(token)]);
  }

  @override
  void endFunctionTypedFormalParameter(Token nameToken, Token? question) {
    events.add(['endFunctionTypedFormalParameter', t(nameToken), t(question)]);
  }

  @override
  void handleIdentifier(Token token, IdentifierContext context) {
    events.add(['handleIdentifier', t(token), context.toString()]);
  }

  @override
  void handleIndexedExpression(Token? question, Token openSquareBracket, Token closeSquareBracket,) {
    events.add(['handleIndexedExpression', t(question), t(openSquareBracket), t(closeSquareBracket)]);
  }

  @override
  void beginIsOperatorType(Token operator) {
    events.add(['beginIsOperatorType', t(operator)]);
  }

  @override
  void endIsOperatorType(Token operator) {
    events.add(['endIsOperatorType', t(operator)]);
  }

  @override
  void handleIsOperator(Token isOperator, Token? not) {
    events.add(['handleIsOperator', t(isOperator), t(not)]);
  }

  @override
  void handleLiteralBool(Token token) {
    events.add(['handleLiteralBool', t(token)]);
  }

  @override
  void handleBreakStatement(bool hasTarget, Token breakKeyword, Token endToken,) {
    events.add(['handleBreakStatement', hasTarget, t(breakKeyword), t(endToken)]);
  }

  @override
  void handleContinueStatement(bool hasTarget, Token continueKeyword, Token endToken,) {
    events.add(['handleContinueStatement', hasTarget, t(continueKeyword), t(endToken)]);
  }

  @override
  void handleEmptyStatement(Token token) {
    events.add(['handleEmptyStatement', t(token)]);
  }

  @override
  void beginAssert(Token assertKeyword, Assert kind) {
    events.add(['beginAssert', t(assertKeyword), kind.name]);
  }

  @override
  void endAssert(Token assertKeyword, Assert kind, Token leftParenthesis, Token? commaToken, Token endToken,) {
    events.add(['endAssert', t(assertKeyword), kind.name, t(leftParenthesis), t(commaToken), t(endToken)]);
  }

  @override
  void handleLiteralDouble(Token token) {
    events.add(['handleLiteralDouble', t(token)]);
  }

  @override
  void handleLiteralDoubleWithSeparators(Token token) {
    events.add(['handleLiteralDoubleWithSeparators', t(token)]);
  }

  @override
  void handleLiteralInt(Token token) {
    events.add(['handleLiteralInt', t(token)]);
  }

  @override
  void handleLiteralIntWithSeparators(Token token) {
    events.add(['handleLiteralIntWithSeparators', t(token)]);
  }

  @override
  void handleLiteralList(int count, Token leftBracket, Token? constKeyword, Token rightBracket,) {
    events.add(['handleLiteralList', count, t(leftBracket), t(constKeyword), t(rightBracket)]);
  }

  @override
  void handleListPattern(int count, Token leftBracket, Token rightBracket) {
    events.add(['handleListPattern', count, t(leftBracket), t(rightBracket)]);
  }

  @override
  void handleLiteralSetOrMap(int count, Token leftBrace, Token? constKeyword, Token rightBrace,) {
    events.add(['handleLiteralSetOrMap', count, t(leftBrace), t(constKeyword), t(rightBrace)]);
  }

  @override
  void handleMapPattern(int count, Token leftBrace, Token rightBrace) {
    events.add(['handleMapPattern', count, t(leftBrace), t(rightBrace)]);
  }

  @override
  void handleLiteralNull(Token token) {
    events.add(['handleLiteralNull', t(token)]);
  }

  @override
  void handleNativeClause(Token nativeToken, bool hasName) {
    events.add(['handleNativeClause', t(nativeToken), hasName]);
  }

  @override
  void handleNamedArgument(Token colon) {
    events.add(['handleNamedArgument', t(colon)]);
  }

  @override
  void handlePositionalArgument(Token token) {
    events.add(['handlePositionalArgument', t(token)]);
  }

  @override
  void handlePatternField(Token? colon) {
    events.add(['handlePatternField', t(colon)]);
  }

  @override
  void handleNamedRecordField(Token colon) {
    events.add(['handleNamedRecordField', t(colon)]);
  }

  @override
  void handlePositionalRecordField(Token token) {
    events.add(['handlePositionalRecordField', t(token)]);
  }

  @override
  void beginNewExpression(Token token) {
    events.add(['beginNewExpression', t(token)]);
  }

  @override
  void endNewExpression(Token token) {
    events.add(['endNewExpression', t(token)]);
  }

  @override
  void handleNoArguments(Token token) {
    events.add(['handleNoArguments', t(token)]);
  }

  @override
  void handleNoConstructorReferenceContinuationAfterTypeArguments(Token token) {
    events.add(['handleNoConstructorReferenceContinuationAfterTypeArguments', t(token)]);
  }

  @override
  void handleNoIdentifier(Token token, IdentifierContext identifierContext) {
    events.add(['handleNoIdentifier', t(token), identifierContext.toString()]);
  }

  @override
  void handleNoTypeNameInConstructorReference(Token token) {
    events.add(['handleNoTypeNameInConstructorReference', t(token)]);
  }

  @override
  void handleNoType(Token lastConsumed) {
    events.add(['handleNoType', t(lastConsumed)]);
  }

  @override
  void handleNoTypeVariables(Token token) {
    events.add(['handleNoTypeVariables', t(token)]);
  }

  @override
  void handleOperator(Token token) {
    events.add(['handleOperator', t(token)]);
  }

  @override
  void handleSwitchCaseNoWhenClause(Token token) {
    events.add(['handleSwitchCaseNoWhenClause', t(token)]);
  }

  @override
  void handleSwitchExpressionCasePattern(Token token) {
    events.add(['handleSwitchExpressionCasePattern', t(token)]);
  }

  @override
  void handleSymbolVoid(Token token) {
    events.add(['handleSymbolVoid', t(token)]);
  }

  @override
  void handleOperatorName(Token operatorKeyword, Token token) {
    events.add(['handleOperatorName', t(operatorKeyword), t(token)]);
  }

  @override
  void handleInvalidOperatorName(Token operatorKeyword, Token token) {
    events.add(['handleInvalidOperatorName', t(operatorKeyword), t(token)]);
  }

  @override
  void handleParenthesizedCondition(Token token, Token? case_, Token? when) {
    events.add(['handleParenthesizedCondition', t(token), t(case_), t(when)]);
  }

  @override
  void beginPattern(Token token) {
    events.add(['beginPattern', t(token)]);
  }

  @override
  void beginPatternGuard(Token when) {
    events.add(['beginPatternGuard', t(when)]);
  }

  @override
  void beginParenthesizedExpressionOrRecordLiteral(Token token) {
    events.add(['beginParenthesizedExpressionOrRecordLiteral', t(token)]);
  }

  @override
  void beginSwitchCaseWhenClause(Token when) {
    events.add(['beginSwitchCaseWhenClause', t(when)]);
  }

  @override
  void endRecordLiteral(Token token, int count, Token? constKeyword) {
    events.add(['endRecordLiteral', t(token), count, t(constKeyword)]);
  }

  @override
  void handleRecordPattern(Token token, int count) {
    events.add(['handleRecordPattern', t(token), count]);
  }

  @override
  void endPattern(Token token) {
    events.add(['endPattern', t(token)]);
  }

  @override
  void endPatternGuard(Token token) {
    events.add(['endPatternGuard', t(token)]);
  }

  @override
  void endParenthesizedExpression(Token token) {
    events.add(['endParenthesizedExpression', t(token)]);
  }

  @override
  void endSwitchCaseWhenClause(Token token) {
    events.add(['endSwitchCaseWhenClause', t(token)]);
  }

  @override
  void handleParenthesizedPattern(Token token) {
    events.add(['handleParenthesizedPattern', t(token)]);
  }

  @override
  void beginConstantPattern(Token? constKeyword) {
    events.add(['beginConstantPattern', t(constKeyword)]);
  }

  @override
  void endConstantPattern(Token? constKeyword) {
    events.add(['endConstantPattern', t(constKeyword)]);
  }

  @override
  void handleObjectPattern(Token firstIdentifier, Token? dot, Token? secondIdentifier,) {
    events.add(['handleObjectPattern', t(firstIdentifier), t(dot), t(secondIdentifier)]);
  }

  @override
  void handleQualified(Token period) {
    events.add(['handleQualified', t(period)]);
  }

  @override
  void handleStringPart(Token token) {
    events.add(['handleStringPart', t(token)]);
  }

  @override
  void handleSuperExpression(Token token, IdentifierContext context) {
    events.add(['handleSuperExpression', t(token), context.toString()]);
  }

  @override
  void beginSwitchCase(int labelCount, int expressionCount, Token beginToken) {
    events.add(['beginSwitchCase', labelCount, expressionCount, t(beginToken)]);
  }

  @override
  void endSwitchCase(int labelCount, int expressionCount, Token? defaultKeyword, Token? colonAfterDefault, int statementCount, Token beginToken, Token endToken,) {
    events.add(['endSwitchCase', labelCount, expressionCount, t(defaultKeyword), t(colonAfterDefault), statementCount, t(beginToken), t(endToken)]);
  }

  @override
  void beginSwitchExpressionCase() {
    events.add(['beginSwitchExpressionCase']);
  }

  @override
  void endSwitchExpressionCase(Token beginToken, Token? when, Token arrow, Token endToken,) {
    events.add(['endSwitchExpressionCase', t(beginToken), t(when), t(arrow), t(endToken)]);
  }

  @override
  void handleThisExpression(Token token, IdentifierContext context) {
    events.add(['handleThisExpression', t(token), context.toString()]);
  }

  @override
  void handleUnaryPostfixAssignmentExpression(Token token) {
    events.add(['handleUnaryPostfixAssignmentExpression', t(token)]);
  }

  @override
  void handleUnaryPrefixExpression(Token token) {
    events.add(['handleUnaryPrefixExpression', t(token)]);
  }

  @override
  void handleRelationalPattern(Token token) {
    events.add(['handleRelationalPattern', t(token)]);
  }

  @override
  void handleUnaryPrefixAssignmentExpression(Token token) {
    events.add(['handleUnaryPrefixAssignmentExpression', t(token)]);
  }

  @override
  void beginFormalParameterDefaultValueExpression() {
    events.add(['beginFormalParameterDefaultValueExpression']);
  }

  @override
  void endFormalParameterDefaultValueExpression() {
    events.add(['endFormalParameterDefaultValueExpression']);
  }

  @override
  void handleValuedFormalParameter(Token equals, Token token, FormalParameterKind kind,) {
    events.add(['handleValuedFormalParameter', t(equals), t(token), kind.name]);
  }

  @override
  void handleFormalParameterWithoutValue(Token token) {
    events.add(['handleFormalParameterWithoutValue', t(token)]);
  }

  @override
  void handleVoidKeyword(Token token) {
    events.add(['handleVoidKeyword', t(token)]);
  }

  @override
  void handleVoidKeywordWithTypeArguments(Token token) {
    events.add(['handleVoidKeywordWithTypeArguments', t(token)]);
  }

  @override
  void beginYieldStatement(Token token) {
    events.add(['beginYieldStatement', t(token)]);
  }

  @override
  void endYieldStatement(Token yieldToken, Token? starToken, Token endToken) {
    events.add(['endYieldStatement', t(yieldToken), t(starToken), t(endToken)]);
  }

  @override
  void endInvalidYieldStatement(Token beginToken, Token? starToken, Token endToken, MessageCode errorCode,) {
    events.add(['endInvalidYieldStatement', t(beginToken), t(starToken), t(endToken), errorCode.name]);
  }

  @override
  void handleRecoverableError(Message message, Token startToken, Token endToken,) {
    events.add(['handleRecoverableError', m(message), t(startToken), t(endToken)]);
    errors.add([message.code.name, startToken.charOffset, endToken.charOffset + endToken.charCount - startToken.charOffset]);
  }

  @override
  void handleExperimentNotEnabled(ExperimentalFlag experimentalFlag, Token beginToken, Token endToken,) {
    events.add(['handleExperimentNotEnabled', experimentalFlag.name, t(beginToken), t(endToken)]);
  }

  @override
  void handleErrorToken(ErrorToken token) {
    events.add(['handleErrorToken', e(token)]);
  }

  @override
  void handleUnescapeError(Message message, Token location, int stringOffset, int length,) {
    events.add(['handleUnescapeError', m(message), t(location), stringOffset, length]);
  }

  @override
  void handleInvalidStatement(Token token, Message message) {
    events.add(['handleInvalidStatement', t(token), m(message)]);
  }

  @override
  void handleScript(Token token) {
    events.add(['handleScript', t(token)]);
  }

  @override
  void handleTypeArgumentApplication(Token openAngleBracket) {
    events.add(['handleTypeArgumentApplication', t(openAngleBracket)]);
  }

  @override
  void handleNewAsIdentifier(Token token) {
    events.add(['handleNewAsIdentifier', t(token)]);
  }

  @override
  void handlePatternVariableDeclarationStatement(Token keyword, Token equals, Token semicolon,) {
    events.add(['handlePatternVariableDeclarationStatement', t(keyword), t(equals), t(semicolon)]);
  }

  @override
  void handlePatternAssignment(Token equals) {
    events.add(['handlePatternAssignment', t(equals)]);
  }

  @override
  void handleDotShorthandContext(Token token) {
    events.add(['handleDotShorthandContext', t(token)]);
  }

  @override
  void handleDotShorthandHead(Token token) {
    events.add(['handleDotShorthandHead', t(token)]);
  }

  @override
  void beginConstDotShorthand(Token token) {
    events.add(['beginConstDotShorthand', t(token)]);
  }

  @override
  void endConstDotShorthand(Token token) {
    events.add(['endConstDotShorthand', t(token)]);
  }

}
