using System.Text;
using Flier.Toolbox.Text;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public class SymbolEncodingConverter : CommonEncodingConverter
    {
        public enum SymbolConvertMode
        {
            HalfSymbolToFullSizeSymbol,
            FullSizeSymbolToHalfSymbol
        }

        public override string Convert(string str, Encoding originEncoding, Encoding targetEncoding)
        {
            throw new NotImplementedException("Use Convert(string origin, SymbolConvertMode mode);");
        }

        public string Convert(string str, SymbolConvertMode mode)
        {
            FastReplace fastReplace = new FastReplace(mode == SymbolConvertMode.HalfSymbolToFullSizeSymbol ? SymbolTable : (SymbolTable.ToLookup(pair => pair.Value, pair => pair.Key).ToDictionary(grp => grp.Key, grp => grp.ToArray()[0])));
            return fastReplace.ReplaceAll(str);
        }

        private static readonly Dictionary<string, string> SymbolTable = new()
        {
            { "," , "，" },
            { "~" , "～" },
            { "!" , "！" },
            { "#" , "＃" },
            { "$" , "＄" },
            { "%" , "％" },
            { "^" , "︿" },
            { "&" , "＆" },
            { "*" , "＊" },
            { "-" , "－" },
            { "+" , "＋" },
            { "{" , "｛" },
            { "}" , "｝" },
            { ";" , "；" },
            { "|" , "｜" },
            { "?" , "？" },
            { "(" , "（" },
            { ")" , "）" },
            { "“" , "「" },
            { "”" , "」" },
            { "‘" , "『" },
            { "’" , "』" },
            { "[" , "［" },
            { "]" , "］" },
            //{ "·" , "．" },
            { " " , "　" },

            { ":" , "：" },
            { "." , "。" },
            { "\"" , "、" },
            { "@" , "＠" },
            { "<" , "＜" },
            { ">" , "＞" },
            { "=" , "＝" },
        };
    }
}