// CPU-only controls run before native compilation, for both invocation modes.
// They mutate only a temporary negative input; the actual source LLVM is never
// renamed, rewritten, or substituted for the native worker input.
json::Object completeEntrySymbolControls(const Input &InputValue, const CompletePlan &P,
                                        StringRef ExpectedSymbol) {
  const std::vector<std::string> Accepted{"A", "_", "a0_Z", std::string(128, 'a')};
  const std::vector<std::string> Refused{
      "", "0bad", "-a", "a-b", "a.b", "a$b", "a/b", "a b", "a\nb",
      std::string("a\0b", 3), "\xc3\xa9", std::string(129, 'a')};
  for (const auto &Name:Accepted)
    require(completeEntrySymbol(Name),"bounded ASCII entry-symbol grammar rejected");
  for (const auto &Name:Refused) {
    require(!completeEntrySymbol(Name),"invalid entry-symbol grammar accepted");
    require(!completeSymbol(InputValue,P,Name),"invalid expected entry symbol accepted");
  }
  const StringRef Wrong=ExpectedSymbol=="_fe2o3_other_entry"?
      StringRef("_fe2o3_another_entry"):StringRef("_fe2o3_other_entry");
  require(completeEntrySymbol(Wrong) && Wrong!=ExpectedSymbol,"symbol refusal control");
  require(!completeSymbol(InputValue,P,Wrong),"mismatched supplied entry symbol accepted");
  const auto Legacy=completeSymbol(InputValue,P);
  require(Legacy.has_value()==(ExpectedSymbol=="complete_body_fixture"),
      "legacy default accepted a different sole kernel");
  const std::string From="@"+ExpectedSymbol.str()+"(";
  const std::string To="@"+Wrong.str()+"(";
  std::string Text(InputValue.Bytes.begin(),InputValue.Bytes.end());
  const size_t At=Text.find(From);
  require(At!=std::string::npos && Text.find(From,At+From.size())==std::string::npos,
      "entry-symbol mutation anchor missing or ambiguous");
  Text.replace(At,From.size(),To);
  Input Changed=InputValue;
  Changed.Bytes.assign(Text.begin(),Text.end());
  Changed.Digest=SHA256::hash(Changed.Bytes);
  require(!completeSymbol(Changed,P,ExpectedSymbol),"substituted LLVM entry symbol accepted");
  return json::Object{
      {"grammar_positive_cases",Accepted.size()},{"grammar_refusal_cases",Refused.size()},
      {"mismatched_expected_symbol_refused",true},{"substituted_llvm_symbol_refused",true},
      {"substituted_llvm_sha256",hex(Changed.Digest)},
      {"legacy_default_preserved",true},{"source_symbol_authentication",false}};
}
