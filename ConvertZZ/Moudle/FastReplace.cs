using System.Collections.Generic;
using System.Linq;
using System.Text;

namespace ConvertZZ
{
    public class FastReplace
    {
        Dictionary<string, string> Dic_ALL;
        Dictionary<char, List<KeyValuePair<string, string>>> Dic_Grouped;

        /// <summary>
        /// 以Dictionary初始化
        /// </summary>
        /// <param name="Dictionary"></param>
        public FastReplace(Dictionary<string, string> Dictionary)
        {
            Dic_ALL = Dictionary;
            Dic_Grouped = Dic_ALL.GroupBy(x => x.Key.First()).ToDictionary(x => x.Key, x => x.ToList());
        }
        public string ReplaceAll(string source)
        {
            StringBuilder sb = new StringBuilder(source.Length);
            char[] key = Dic_Grouped.Keys.ToArray();
            int indexPass = 0, indexNow = 0;
            indexPass = indexNow;
            indexNow = source.IndexOfAny(key, indexNow);
            while (indexNow != -1)
            {
                sb.Append(source.Substring(indexPass, indexNow - indexPass));
                var _dic = Dic_Grouped[source[indexNow]];
                bool replaced = false;
                foreach (var a in _dic)
                {
                    if (indexNow + a.Key.Length > source.Length)
                        continue;
                    if (source.Substring(indexNow, a.Key.Length) == a.Key)
                    {
                        sb.Append(a.Value);
                        indexNow += a.Key.Length;
                        replaced = true;
                    }
                }
                if (!replaced)
                {
                    sb.Append(source.Substring(indexNow, 1));
                    indexNow++;
                }
                indexPass = indexNow;
                indexNow = source.IndexOfAny(key, indexNow);
            }
            sb.Append(source.Substring(indexPass, source.Length - indexPass));
            return sb.ToString();
        }
    }
}
