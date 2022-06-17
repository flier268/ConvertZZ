using System.Text;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public class UnicodeToHtmlHexCodeEncodingConverter : CommonEncodingConverter
    {
        public override string Convert(string str, Encoding originEncoding, Encoding targetEncoding)
        {
            return Convert(str);
        }

        public string Convert(string str)
        {
            StringBuilder sb = new();
            foreach (char c in str)
            {
                if ((' ' <= c && c <= '~') || (c == '\r') || (c == '\n'))
                {
                    if (c == '&')
                    {
                        sb.Append("&amp;");
                    }
                    else if (c == '<')
                    {
                        sb.Append("&lt;");
                    }
                    else if (c == '>')
                    {
                        sb.Append("&gt;");
                    }
                    else
                    {
                        sb.Append(c.ToString());
                    }
                }
                else
                {
                    sb.Append("&#");
                    sb.Append((int)c);
                    sb.Append(';');
                }
            }
            return sb.ToString();
        }
    }
}