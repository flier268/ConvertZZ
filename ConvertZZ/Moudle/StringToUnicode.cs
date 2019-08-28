using System.Linq;
using System.Text;

namespace ConvertZZ.Moudle
{
    public class StringToUnicode
    {
        public static string TryToConvertLatin1ToUnicode(string str, Encoding encoding)
        {
            if (CalcRate(str.ToArray(), Latin1) > 0.2)
            {
                return encoding.GetString(Encoding.GetEncoding("ISO-8859-1").GetBytes(str));
            }
            else
                return str;
        }

        private static double CalcRate(char[] Array, char[] Table)
        {
            double count = Array.Count(x => Table.Contains(x));
            return count / Array.Length;
        }

        private static readonly char[] Latin1 = new char[] {
            '¡','¢','£','¤','¥','¦','§','¨','©','ª','«','¬','®','¯',
            '°','±','²','³','´','µ','¶','·','¸','¹','º','»','¼','½','¾','¿',
            'À','Á','Â','Ã','Ä','Å','Æ','Ç','È','É','Ê','Ë','Ì','Í','Î','Ï',
            'Ð','Ñ','Ò','Ó','Ô','Õ','Ö','×','Ø','Ù','Ú','Û','Ü','Ý','Þ','ß',
            'à','á','â','ã','ä','å','æ','ç','è','é','ê','ë','ì','í','î','ï',
            'ð','ñ','ò','ó','ô','õ','ö','÷','ø','ù','ú','û','ü','ý','þ','ÿ'
        };
    }
}
