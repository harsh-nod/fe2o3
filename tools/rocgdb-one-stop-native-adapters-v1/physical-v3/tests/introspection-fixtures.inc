/* SPDX-License-Identifier: GPL-3.0-or-later
   Inert shells, not GDB objects/custody. Only the two marked method definitions
   are exact source copies; the native GDB build must compile the real classes. */
#include <cstdio>
#include <string>
#include <typeinfo>
struct ui_file { virtual ~ui_file ()=default; };
class stdio_file : public ui_file {
public:
  explicit stdio_file (FILE *file,bool own=false):m_file(file),m_close_p(own) {}
  bool one_stop_borrowed_stdio_matches (FILE *expected) const noexcept
  {
    return typeid (*this)==typeid (stdio_file) && expected!=nullptr
      && m_file==expected && !m_close_p;
  }

private:
  FILE *m_file; bool m_close_p;
};
class pager_file : public ui_file {
public:
  explicit pager_file (ui_file *stream):m_stream(stream) {}
  void fixture_state (bool paging,const char *wrap,int column,int indent) {
    m_paging=paging;m_wrap_buffer=wrap;m_wrap_column=column;m_wrap_indent=indent;
  }
  bool one_stop_borrowed_output_matches (FILE *expected) const noexcept
  {
    if (typeid (*this)!=typeid (pager_file) || m_paging
        || !m_wrap_buffer.empty () || m_wrap_column!=0 || m_wrap_indent!=0)
      return false;
    const auto *stdio=dynamic_cast<const stdio_file *> (m_stream);
    return stdio!=nullptr && stdio->one_stop_borrowed_stdio_matches (expected);
  }

private:
  ui_file *m_stream; std::string m_wrap_buffer; int m_wrap_column=0,m_wrap_indent=0;
  bool m_paging=false;
};
