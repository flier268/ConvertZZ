using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace ConvertZZ
{
    public class TransferEncodeHelper
    {
        public static string TransferStr(string str, Encoding originalEncode, Encoding targetEncode)
        {
            try
            {
                byte[] c=targetEncode.GetBytes(str);
                return originalEncode.GetString(c);
            }
            catch
            {
                Console.WriteLine("There is an exception.");
                return "";
            }
        }
    }
}
